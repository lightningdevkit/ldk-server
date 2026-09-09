// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use corepc_node::Node;
use ldk_node::bitcoin::Amount;
use ldk_server_client::client::{EventStream, LdkServerClient};
use ldk_server_client::error::LdkServerErrorCode;
use ldk_server_client::ldk_server_grpc::api::{GetNodeInfoRequest, GetNodeInfoResponse};
use ldk_server_client::ldk_server_grpc::events::event_envelope::Event;
use ldk_server_client::ldk_server_grpc::events::EventEnvelope;
use ldk_server_grpc::api::{
	open_channel_request, Bolt11ReceiveRequest, Bolt11SendRequest, CloseChannelRequest,
	GetBalancesRequest, GetBalancesResponse, GraphGetChannelRequest, GraphListChannelsRequest,
	ListChannelsRequest, ListForwardedPaymentsRequest, ListPaymentsRequest, OnchainReceiveRequest,
	OpenChannelRequest,
};
use ldk_server_grpc::types::{
	lightning_balance, payment_kind, pending_sweep_balance, BalanceSource, Channel,
	ClaimableAwaitingConfirmations, ForwardedPayment, LightningBalance, Payment, PaymentDirection,
	PaymentStatus,
};
use serde_json::{json, Value};

const EVENT_TIMEOUT: Duration = Duration::from_secs(15);

/// Wrapper around a managed bitcoind process for regtest.
pub struct TestBitcoind {
	pub bitcoind: Node,
}

impl Default for TestBitcoind {
	fn default() -> Self {
		Self::new()
	}
}

impl TestBitcoind {
	pub fn new() -> Self {
		Self::with_extra_args(&[])
	}

	/// Same as [`TestBitcoind::new`], but starts bitcoind with `-rest=1` so its REST interface
	/// (disabled by default) is reachable, for exercising `[bitcoind]`'s `rest_address` option.
	pub fn new_with_rest() -> Self {
		Self::with_extra_args(&["-rest=1"])
	}

	fn with_extra_args(extra_args: &[&str]) -> Self {
		let mut conf = corepc_node::Conf::default();
		conf.args.extend_from_slice(extra_args);

		let bitcoind = match std::env::var("BITCOIND_EXE") {
			Ok(path) => Node::with_conf(path, &conf).unwrap(),
			Err(_) => Node::from_downloaded_with_conf(&conf).unwrap(),
		};
		// Generate initial blocks to make coins spendable
		let address = bitcoind.client.new_address().unwrap();
		bitcoind.client.generate_to_address(101, &address).unwrap();
		Self { bitcoind }
	}

	pub fn mine_blocks(&self, count: u64) {
		let address = self.bitcoind.client.new_address().unwrap();
		self.bitcoind.client.generate_to_address(count as usize, &address).unwrap();
	}

	pub fn fund_address(&self, addr: &str, btc_amount: f64) {
		use corepc_node::client::bitcoin::{Address, Amount};
		let address: Address<corepc_node::client::bitcoin::address::NetworkUnchecked> =
			addr.parse().unwrap();
		let address = address.assume_checked();
		let amount = Amount::from_btc(btc_amount).unwrap();
		self.bitcoind.client.send_to_address(&address, amount).unwrap();
		self.mine_blocks(1);
	}

	pub fn rpc_url(&self) -> String {
		self.bitcoind.rpc_url()
	}

	pub fn rpc_cookie(&self) -> PathBuf {
		self.bitcoind.params.cookie_file.clone()
	}

	/// Returns (host, port, user, password) for the bitcoind RPC.
	pub fn rpc_details(&self) -> (String, u16, String, String) {
		let rpc_url = self.rpc_url();
		let rpc_address = rpc_url.strip_prefix("http://").unwrap_or(&rpc_url);
		let rpc_parts: Vec<&str> = rpc_address.splitn(2, ':').collect();
		let host = rpc_parts[0].to_string();
		let port: u16 = rpc_parts[1].parse().unwrap();

		let cookie_content = std::fs::read_to_string(self.rpc_cookie()).unwrap();
		let mut parts = cookie_content.splitn(2, ':');
		let user = parts.next().unwrap().to_string();
		let password = parts.next().unwrap().to_string();

		(host, port, user, password)
	}
}

/// Handle to a running ldk-server child process.
pub struct LdkServerHandle {
	child: Option<Child>,
	pub grpc_port: u16,
	pub p2p_port: u16,
	pub storage_dir: PathBuf,
	pub config_path: PathBuf,
	pub macaroon: String,
	pub tls_cert_path: PathBuf,
	pub node_id: String,
	client: LdkServerClient,
}

#[derive(Default)]
pub struct LdkServerConfig {
	pub metrics_auth: Option<(String, String)>,
}

/// Dynamic parameters available when building test configs.
pub struct TestServerParams {
	pub grpc_port: u16,
	pub p2p_port: u16,
	pub storage_dir: PathBuf,
	pub rpc_address: String,
	pub rpc_user: String,
	pub rpc_password: String,
}

/// A chain source for the test config, mirroring the server's supported backends.
pub enum ChainSource {
	Bitcoind {
		rpc_address: String,
		rpc_user: String,
		rpc_password: String,
		rest_address: Option<String>,
	},
	Electrum {
		server_url: String,
	},
	Esplora {
		server_url: String,
	},
}

impl ChainSource {
	/// Render the chain source as its TOML section.
	fn to_toml(&self) -> String {
		match self {
			ChainSource::Bitcoind { rpc_address, rpc_user, rpc_password, rest_address } => {
				let mut toml = format!(
					"[bitcoind]\nrpc_address = \"{}\"\nrpc_user = \"{}\"\nrpc_password = \"{}\"",
					rpc_address, rpc_user, rpc_password
				);
				if let Some(rest_address) = rest_address {
					toml.push_str(&format!("\nrest_address = \"{}\"", rest_address));
				}
				toml
			},
			ChainSource::Electrum { server_url } => {
				format!("[electrum]\nserver_url = \"{}\"", server_url)
			},
			ChainSource::Esplora { server_url } => {
				format!("[esplora]\nserver_url = \"{}\"", server_url)
			},
		}
	}
}

/// Builder for the ldk-server config TOML used in tests.
///
/// Tests tweak named, typed knobs and call [`TestConfigBuilder::build`] once to
/// produce the TOML. This keeps tests from doing string surgery on rendered output.
pub struct TestConfigBuilder {
	listening_addresses: Vec<String>,
	announcement_addresses: Vec<String>,
	grpc_service_address: String,
	alias: Option<String>,
	storage_dir: PathBuf,
	postgres: Option<(String, String)>,
	chain_source: ChainSource,
	metrics_auth: Option<(String, String)>,
	log: Option<(Option<String>, String)>,
	tls_hosts: Option<Vec<String>>,
	forwarded_payment_tracking_mode: Option<String>,
}

impl TestConfigBuilder {
	/// Start from the default test config: a single localhost listening address, the
	/// `e2e-test-node` alias, and a bitcoind RPC chain source derived from `params`.
	pub fn new(params: &TestServerParams) -> Self {
		Self {
			listening_addresses: vec![format!("127.0.0.1:{}", params.p2p_port)],
			announcement_addresses: Vec::new(),
			grpc_service_address: format!("127.0.0.1:{}", params.grpc_port),
			alias: Some("e2e-test-node".to_string()),
			storage_dir: params.storage_dir.clone(),
			postgres: None,
			chain_source: ChainSource::Bitcoind {
				rpc_address: params.rpc_address.clone(),
				rpc_user: params.rpc_user.clone(),
				rpc_password: params.rpc_password.clone(),
				rest_address: None,
			},
			metrics_auth: None,
			log: None,
			tls_hosts: None,
			forwarded_payment_tracking_mode: None,
		}
	}

	pub fn forwarded_payment_tracking_mode(mut self, mode: &str) -> Self {
		self.forwarded_payment_tracking_mode = Some(mode.to_string());
		self
	}

	/// Store LDK Node state in PostgreSQL, keeping keys and server files on disk.
	pub fn postgres(mut self, connection_string: &str, kv_table_name: &str) -> Self {
		self.postgres = Some((connection_string.to_string(), kv_table_name.to_string()));
		self
	}

	/// Set the node alias, or `None` to omit it entirely.
	pub fn alias(mut self, alias: Option<&str>) -> Self {
		self.alias = alias.map(str::to_string);
		self
	}

	/// Set the listening addresses. An empty vec omits the key entirely.
	pub fn listening_addresses(mut self, addresses: Vec<String>) -> Self {
		self.listening_addresses = addresses;
		self
	}

	/// Set the announcement addresses. An empty vec (the default) omits the key.
	pub fn announcement_addresses(mut self, addresses: Vec<String>) -> Self {
		self.announcement_addresses = addresses;
		self
	}

	/// Replace the chain source backend.
	pub fn chain_source(mut self, chain_source: ChainSource) -> Self {
		self.chain_source = chain_source;
		self
	}

	/// Add HTTP basic auth credentials to the `[metrics]` section.
	pub fn metrics_auth(mut self, username: &str, password: &str) -> Self {
		self.metrics_auth = Some((username.to_string(), password.to_string()));
		self
	}

	/// Add a `[log]` section with the given file path and optional level.
	pub fn log(mut self, level: Option<&str>, file: &str) -> Self {
		self.log = Some((level.map(str::to_string), file.to_string()));
		self
	}

	/// Add a `[tls]` section advertising the given hosts.
	pub fn tls_hosts(mut self, hosts: Vec<String>) -> Self {
		self.tls_hosts = Some(hosts);
		self
	}

	/// Build the config into a TOML string.
	pub fn build(&self) -> String {
		fn toml_string_array(values: &[String]) -> String {
			let quoted: Vec<String> = values.iter().map(|v| format!("\"{}\"", v)).collect();
			format!("[{}]", quoted.join(", "))
		}

		let mut node = vec!["[node]".to_string(), "network = \"regtest\"".to_string()];
		if let Some(mode) = &self.forwarded_payment_tracking_mode {
			node.push(format!("forwarded_payment_tracking_mode = \"{mode}\""));
		}
		if !self.listening_addresses.is_empty() {
			node.push(format!(
				"listening_addresses = {}",
				toml_string_array(&self.listening_addresses)
			));
		}
		node.push(format!("grpc_service_address = \"{}\"", self.grpc_service_address));
		if let Some(alias) = &self.alias {
			node.push(format!("alias = \"{}\"", alias));
		}
		if !self.announcement_addresses.is_empty() {
			node.push(format!(
				"announcement_addresses = {}",
				toml_string_array(&self.announcement_addresses)
			));
		}

		let metrics_auth = match &self.metrics_auth {
			Some((user, pass)) => {
				format!("\nusername = \"{}\"\npassword = \"{}\"", user, pass)
			},
			None => String::new(),
		};

		let mut config = format!(
			r#"{node}

[storage.disk]
dir_path = "{storage_dir}"

{chain_source}

[liquidity.lsps2_service]
advertise_service = false
channel_opening_fee_ppm = 10000
channel_over_provisioning_ppm = 100000
min_channel_opening_fee_msat = 0
min_channel_lifetime = 100
max_client_to_self_delay = 1024
min_payment_size_msat = 0
max_payment_size_msat = 1000000000
client_trusts_lsp = true
disable_client_reserve = false

[metrics]
enabled = true
poll_metrics_interval = 1{metrics_auth}
"#,
			node = node.join("\n"),
			storage_dir = self.storage_dir.display(),
			chain_source = self.chain_source.to_toml(),
			metrics_auth = metrics_auth,
		);

		if let Some((connection_string, kv_table_name)) = &self.postgres {
			config.push_str(&format!(
				"\n[storage.postgres]\nconnection_string = \"{}\"\nkv_table_name = \"{}\"\n",
				connection_string, kv_table_name,
			));
		}

		if let Some((level, file)) = &self.log {
			config.push_str("\n[log]\n");
			if let Some(level) = level {
				config.push_str(&format!("level = \"{}\"\n", level));
			}
			config.push_str(&format!("file = \"{}\"\n", file));
		}

		if let Some(hosts) = &self.tls_hosts {
			config.push_str(&format!("\n[tls]\nhosts = {}\n", toml_string_array(hosts)));
		}

		config
	}
}

impl LdkServerHandle {
	/// Starts a new ldk-server instance against the given bitcoind.
	/// Waits until the server is ready to accept requests.
	pub async fn start(bitcoind: &TestBitcoind) -> Self {
		Self::start_with_options(bitcoind, LdkServerConfig::default()).await
	}

	pub async fn start_with_options(bitcoind: &TestBitcoind, config: LdkServerConfig) -> Self {
		Self::start_with_config(bitcoind, |params| {
			let mut builder = TestConfigBuilder::new(params);
			if let Some((user, pass)) = &config.metrics_auth {
				builder = builder.metrics_auth(user, pass);
			}
			builder.build()
		})
		.await
	}

	pub async fn start_with_config(
		config_bitcoind: &TestBitcoind, config: impl FnOnce(&TestServerParams) -> String,
	) -> Self {
		let (mut child, params, config_path) = spawn_server(config_bitcoind, config);
		forward_server_output(&mut child);
		let TestServerParams { grpc_port, p2p_port, storage_dir, .. } = params;

		// Wait for the admin macaroon and TLS certificate files to appear.
		let network_dir = storage_dir.join("regtest");
		let macaroon_path = network_dir.join("macaroons").join("admin.macaroon");
		let tls_cert_path = storage_dir.join("tls.crt");

		wait_for_file(&macaroon_path, Duration::from_secs(30)).await;
		wait_for_file(&tls_cert_path, Duration::from_secs(30)).await;

		let macaroon = std::fs::read_to_string(&macaroon_path).unwrap().trim().to_string();

		// Read TLS cert
		let tls_cert_pem = std::fs::read(&tls_cert_path).unwrap();

		let base_url = format!("127.0.0.1:{grpc_port}");
		let client = LdkServerClient::new(base_url, macaroon.clone(), &tls_cert_pem).unwrap();

		let mut handle = Self {
			child: Some(child),
			grpc_port,
			p2p_port,
			storage_dir,
			config_path,
			macaroon,
			tls_cert_path,
			node_id: String::new(),
			client,
		};

		// Wait for server to be ready and get node info
		let node_info = wait_for_server_ready(&handle, Duration::from_secs(60)).await;
		handle.node_id = node_info.node_id;

		handle
	}

	/// Kill and restart the server with the same config and storage to test crash recovery.
	pub async fn restart(&mut self) {
		let mut child = self.child.take().expect("Server is not running");
		child.kill().expect("Failed to kill ldk-server");
		child.wait().expect("Failed to reap ldk-server");
		let mut child = spawn_server_process(&self.config_path);
		forward_server_output(&mut child);
		self.child = Some(child);
		let info = wait_for_server_ready(self, Duration::from_secs(60)).await;
		assert_eq!(info.node_id, self.node_id, "Node identity changed after restart");
	}

	pub fn client(&self) -> &LdkServerClient {
		&self.client
	}

	pub fn node_id(&self) -> &str {
		&self.node_id
	}

	pub fn base_url(&self) -> String {
		format!("127.0.0.1:{}", self.grpc_port)
	}
}

impl Drop for LdkServerHandle {
	fn drop(&mut self) {
		if let Some(mut child) = self.child.take() {
			let _ = child.kill();
			let _ = child.wait();
		}
	}
}

/// Prepare test server params and spawn the ldk-server process.
fn spawn_server(
	bitcoind: &TestBitcoind, config_fn: impl FnOnce(&TestServerParams) -> String,
) -> (Child, TestServerParams, PathBuf) {
	#[allow(deprecated)]
	let storage_dir = tempfile::tempdir().unwrap().into_path();
	let grpc_port = find_available_port();
	let p2p_port = find_available_port();

	let (rpc_host, rpc_port_num, rpc_user, rpc_password) = bitcoind.rpc_details();
	let rpc_address = format!("{rpc_host}:{rpc_port_num}");

	let params =
		TestServerParams { grpc_port, p2p_port, storage_dir, rpc_address, rpc_user, rpc_password };

	let config_content = config_fn(&params);

	let config_path = params.storage_dir.join("config.toml");
	std::fs::write(&config_path, &config_content).unwrap();

	let child = spawn_server_process(&config_path);
	(child, params, config_path)
}

/// Spawn a server using an existing config, retaining its output pipes.
fn spawn_server_process(config_path: &Path) -> Child {
	let server_binary = server_binary_path();
	Command::new(&server_binary)
		.arg(config_path)
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.unwrap_or_else(|e| {
			panic!("Failed to start ldk-server binary at {:?}: {}", server_binary, e)
		})
}

fn forward_server_output(child: &mut Child) {
	// Spawn threads to forward stdout and stderr for debugging
	let stdout = child.stdout.take().unwrap();
	std::thread::spawn(move || {
		let reader = BufReader::new(stdout);
		for line in reader.lines().map_while(Result::ok) {
			eprintln!("[ldk-server stdout] {}", line);
		}
	});
	let stderr = child.stderr.take().unwrap();
	std::thread::spawn(move || {
		let reader = BufReader::new(stderr);
		for line in reader.lines().map_while(Result::ok) {
			if line.contains("Failed to retrieve fee rate estimates") {
				continue;
			}
			eprintln!("[ldk-server stderr] {}", line);
		}
	});
}

/// Start ldk-server with the given config and expect it to fail (exit non-zero).
/// Returns the stderr output for assertion in tests.
pub fn start_expect_failure(
	bitcoind: &TestBitcoind, config_fn: impl FnOnce(&TestServerParams) -> String,
) -> String {
	let (mut child, ..) = spawn_server(bitcoind, config_fn);

	let timeout = Duration::from_secs(30);
	let start = std::time::Instant::now();
	loop {
		match child.try_wait() {
			Ok(Some(_)) => break,
			Ok(None) => {
				if start.elapsed() > timeout {
					let _ = child.kill();
					panic!(
						"Server did not exit within {:?} — it may have started successfully \
						 instead of failing",
						timeout
					);
				}
				std::thread::sleep(Duration::from_millis(100));
			},
			Err(e) => panic!("Failed to wait for ldk-server process: {}", e),
		}
	}

	let output = child
		.wait_with_output()
		.unwrap_or_else(|e| panic!("Failed to read ldk-server output: {}", e));

	assert!(
		!output.status.success(),
		"Expected server to fail but it exited with status: {}",
		output.status
	);

	String::from_utf8_lossy(&output.stderr).to_string()
}
/// Find an available TCP port by binding to port 0.
pub fn find_available_port() -> u16 {
	let listener = TcpListener::bind("127.0.0.1:0").unwrap();
	listener.local_addr().unwrap().port()
}

/// Wait for a file to exist on disk, polling every 100ms.
pub async fn wait_for_file(path: &Path, timeout: Duration) {
	let start = std::time::Instant::now();
	while !path.exists() {
		if start.elapsed() > timeout {
			panic!("Timed out waiting for file: {:?}", path);
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
}

/// Wait for the next event that matches the predicate.
pub async fn wait_for_event(
	events: &mut EventStream, pred: impl Fn(&Event) -> bool,
) -> EventEnvelope {
	tokio::time::timeout(EVENT_TIMEOUT, async {
		while let Some(Ok(event)) = events.next_message().await {
			if event.event.as_ref().is_some_and(&pred) {
				return event;
			}
		}
		panic!("Event stream ended without matching event");
	})
	.await
	.expect("Timed out waiting for event")
}

/// Poll get_node_info until the server responds successfully.
async fn wait_for_server_ready(handle: &LdkServerHandle, timeout: Duration) -> GetNodeInfoResponse {
	let start = std::time::Instant::now();
	loop {
		match handle.client().get_node_info(GetNodeInfoRequest {}).await {
			Ok(info) => return info,
			Err(_) => {
				if start.elapsed() > timeout {
					panic!("Timed out waiting for ldk-server to become ready");
				}
				tokio::time::sleep(Duration::from_millis(500)).await;
			},
		}
	}
}

/// Returns the path to the ldk-server binary (built automatically by build.rs).
pub fn server_binary_path() -> PathBuf {
	PathBuf::from(env!("LDK_SERVER_BIN"))
}

/// Returns the path to the ldk-server-cli binary (built automatically by build.rs).
pub fn cli_binary_path() -> PathBuf {
	PathBuf::from(env!("LDK_SERVER_CLI_BIN"))
}

/// Returns the path to the ldk-server-mcp binary (built automatically by build.rs).
pub fn mcp_binary_path() -> PathBuf {
	PathBuf::from(env!("LDK_SERVER_MCP_BIN"))
}

/// Handle to a running ldk-server-mcp child process.
pub struct McpHandle {
	child: Option<Child>,
	stdin: std::process::ChildStdin,
	stdout: BufReader<std::process::ChildStdout>,
}

impl McpHandle {
	pub fn start(server: &LdkServerHandle) -> Self {
		Self::start_with_macaroon(server, &server.macaroon)
	}

	pub fn start_with_macaroon(server: &LdkServerHandle, macaroon: &str) -> Self {
		let mcp_path = mcp_binary_path();
		let mut child = Command::new(&mcp_path)
			.env("LDK_BASE_URL", server.base_url())
			.env("LDK_MACAROON", macaroon)
			.env("LDK_TLS_CERT_PATH", server.tls_cert_path.to_str().unwrap())
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn()
			.unwrap_or_else(|e| panic!("Failed to run MCP server at {:?}: {}", mcp_path, e));

		let stdin = child.stdin.take().unwrap();
		let stdout = BufReader::new(child.stdout.take().unwrap());

		Self { child: Some(child), stdin, stdout }
	}

	pub fn send(&mut self, request: &Value) {
		let line = serde_json::to_string(request).unwrap();
		writeln!(self.stdin, "{}", line).unwrap();
		self.stdin.flush().unwrap();
	}

	pub fn recv(&mut self) -> Value {
		let mut line = String::new();
		self.stdout.read_line(&mut line).expect("Failed to read MCP stdout");
		serde_json::from_str(line.trim()).expect("Failed to parse MCP response")
	}

	pub fn call(&mut self, id: u64, method: &str, params: Value) -> Value {
		self.send(&serde_json::json!({
			"jsonrpc": "2.0",
			"id": id,
			"method": method,
			"params": params,
		}));
		self.recv()
	}
}

impl Drop for McpHandle {
	fn drop(&mut self) {
		if let Some(mut child) = self.child.take() {
			let _ = child.kill();
			let _ = child.wait();
		}
	}
}

/// Run a CLI command against the given server handle and return raw stdout as a string.
pub fn run_cli_raw(handle: &LdkServerHandle, args: &[&str]) -> String {
	let cli_path = cli_binary_path();
	let output = Command::new(&cli_path)
		.arg("--base-url")
		.arg(handle.base_url())
		.arg("--macaroon")
		.arg(&handle.macaroon)
		.arg("--tls-cert")
		.arg(handle.tls_cert_path.to_str().unwrap())
		.args(args)
		.output()
		.unwrap_or_else(|e| panic!("Failed to run CLI at {:?}: {}", cli_path, e));

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		let stdout = String::from_utf8_lossy(&output.stdout);
		panic!(
			"CLI command {:?} failed with status {}\nstdout: {}\nstderr: {}",
			args, output.status, stdout, stderr
		);
	}

	String::from_utf8(output.stdout).unwrap()
}

/// Run a CLI command using the server's config file for connection details.
pub fn run_cli_with_config_raw(handle: &LdkServerHandle, args: &[&str]) -> String {
	let cli_path = cli_binary_path();
	let output = Command::new(&cli_path)
		.arg("--config")
		.arg(handle.config_path.to_str().unwrap())
		.args(args)
		.output()
		.unwrap_or_else(|e| panic!("Failed to run CLI at {:?}: {}", cli_path, e));

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		let stdout = String::from_utf8_lossy(&output.stdout);
		panic!(
			"CLI command {:?} failed with status {}\nstdout: {}\nstderr: {}",
			args, output.status, stdout, stderr
		);
	}

	String::from_utf8(output.stdout).unwrap()
}

/// Run a CLI command against the given server handle and return parsed JSON output.
pub fn run_cli(handle: &LdkServerHandle, args: &[&str]) -> serde_json::Value {
	let stdout = run_cli_raw(handle, args);
	serde_json::from_str(&stdout)
		.unwrap_or_else(|e| panic!("Failed to parse CLI output as JSON: {e}\nOutput: {stdout}"))
}

/// Run a CLI command using the server's config file and return parsed JSON output.
pub fn run_cli_with_config(handle: &LdkServerHandle, args: &[&str]) -> serde_json::Value {
	let stdout = run_cli_with_config_raw(handle, args);
	serde_json::from_str(&stdout)
		.unwrap_or_else(|e| panic!("Failed to parse CLI output as JSON: {e}\nOutput: {stdout}"))
}

/// Mine blocks and wait for all servers to sync to the new chain tip.
pub async fn mine_and_sync(
	bitcoind: &TestBitcoind, servers: &[&LdkServerHandle], block_count: u64,
) {
	bitcoind.mine_blocks(block_count);

	let expected_height = bitcoind.bitcoind.client.get_block_count().unwrap().0;

	for server in servers {
		let client = server.client();
		let timeout = Duration::from_secs(30);
		let start = std::time::Instant::now();
		loop {
			if let Ok(info) = client.get_node_info(GetNodeInfoRequest {}).await {
				if info.current_best_block.as_ref().map(|b| b.height).unwrap_or(0)
					>= expected_height as u32
				{
					break;
				}
			}
			if start.elapsed() > timeout {
				panic!(
					"Timed out waiting for server {} to sync to height {}",
					server.node_id(),
					expected_height
				);
			}
			tokio::time::sleep(Duration::from_millis(500)).await;
		}
	}
}

/// Wait for a transaction to enter the mempool and return its decoded details.
pub async fn wait_for_transaction(bitcoind: &TestBitcoind, txid: &str) -> Value {
	tokio::time::timeout(Duration::from_secs(30), async {
		loop {
			let mempool: Vec<String> = bitcoind.bitcoind.client.call("getrawmempool", &[]).unwrap();
			if mempool.iter().any(|id| id == txid) {
				return bitcoind
					.bitcoind
					.client
					.call("getrawtransaction", &[json!(txid), json!(true)])
					.unwrap();
			}
			tokio::time::sleep(Duration::from_millis(100)).await;
		}
	})
	.await
	.expect("transaction did not enter the mempool")
}

/// Wait for the on-chain wallet to complete another sync.
///
/// The pinned wallet records a replacement before its background sync sees the transaction.
pub async fn wait_for_wallet_sync(server: &LdkServerHandle) {
	let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
	tokio::time::timeout(Duration::from_secs(30), async {
		loop {
			let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
			if info.latest_onchain_wallet_sync_timestamp.is_some_and(|timestamp| timestamp > after)
			{
				break;
			}
			tokio::time::sleep(Duration::from_millis(100)).await;
		}
	})
	.await
	.expect("wallet did not sync after the replacement");
}

/// Wait for a transaction to appear in the node's payment history.
pub async fn payment_for_tx(server: &LdkServerHandle, txid: &str) -> Payment {
	tokio::time::timeout(Duration::from_secs(30), async {
		loop {
			for payment in list_payments(server).await {
				if let Some(payment_kind::Kind::Onchain(onchain)) =
					payment.kind.as_ref().and_then(|kind| kind.kind.as_ref())
				{
					if onchain.txid == txid {
						return payment;
					}
				}
			}
			tokio::time::sleep(Duration::from_millis(100)).await;
		}
	})
	.await
	.expect("payment was not recorded")
}

/// Check that a replacement preserves the recipient amount.
pub async fn assert_replacement(
	bitcoind: &TestBitcoind, old: &str, new: &str, address: &str, amount: Amount,
) {
	assert_ne!(old, new);
	let tx = wait_for_transaction(bitcoind, new).await;
	let recipient = tx["vout"]
		.as_array()
		.unwrap()
		.iter()
		.find(|output| output["scriptPubKey"]["address"] == address)
		.unwrap();
	assert_eq!(Amount::from_btc(recipient["value"].as_f64().unwrap()).unwrap(), amount);
	let mempool: Vec<String> = bitcoind.bitcoind.client.call("getrawmempool", &[]).unwrap();
	assert!(!mempool.iter().any(|id| id == old));
}

/// Wait until the given client has at least one usable channel,
/// periodically mining blocks to trigger chain sync.
pub async fn wait_for_usable_channel(
	client: &LdkServerClient, bitcoind: &TestBitcoind, timeout: Duration,
) {
	let start = std::time::Instant::now();
	loop {
		let channels = client.list_channels(ListChannelsRequest {}).await.unwrap();
		if channels.channels.iter().any(|c| c.is_usable) {
			return;
		}
		if start.elapsed() > timeout {
			let chan_info: Vec<_> = channels
				.channels
				.iter()
				.map(|c| {
					format!(
						"id={} is_ready={} is_usable={} value={}",
						c.user_channel_id, c.is_channel_ready, c.is_usable, c.channel_value_sats
					)
				})
				.collect();
			panic!("Timed out waiting for usable channel. Channels: {:?}", chan_info);
		}
		// Mine a block to trigger chain sync in the LDK nodes
		bitcoind.mine_blocks(1);
		tokio::time::sleep(Duration::from_secs(1)).await;
	}
}

/// Wait for a server's on-chain wallet to have confirmed balance.
pub async fn wait_for_onchain_balance(client: &LdkServerClient, timeout: Duration) {
	let start = std::time::Instant::now();
	loop {
		let bal = client.get_balances(GetBalancesRequest {}).await.unwrap();
		if bal.spendable_onchain_balance_sats > 0 {
			return;
		}
		if start.elapsed() > timeout {
			panic!("Timed out waiting for on-chain balance");
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	}
}

/// Fund both servers' on-chain wallets, open a channel from A to B,
/// mine to confirm, and wait until it's usable.
pub async fn setup_funded_channel(
	bitcoind: &TestBitcoind, server_a: &LdkServerHandle, server_b: &LdkServerHandle,
	channel_amount_sats: u64,
) -> String {
	// Fund both servers (server B needs on-chain reserves for anchor channels)
	let addr_a = server_a.client().onchain_receive(OnchainReceiveRequest {}).await.unwrap().address;
	let addr_b = server_b.client().onchain_receive(OnchainReceiveRequest {}).await.unwrap().address;
	bitcoind.fund_address(&addr_a, 1.0);
	bitcoind.fund_address(&addr_b, 0.1);
	mine_and_sync(bitcoind, &[server_a, server_b], 6).await;

	// Wait for both servers to see their on-chain balance
	wait_for_onchain_balance(server_a.client(), Duration::from_secs(30)).await;
	wait_for_onchain_balance(server_b.client(), Duration::from_secs(30)).await;

	// Open channel A -> B
	let open_resp = server_a
		.client()
		.open_channel(OpenChannelRequest {
			node_pubkey: server_b.node_id().to_string(),
			address: format!("127.0.0.1:{}", server_b.p2p_port),
			amount: Some(open_channel_request::Amount::ChannelAmountSats(channel_amount_sats)),
			push_to_counterparty_msat: None,
			channel_config: None,
			announce_channel: true,
			disable_counterparty_reserve: false,
		})
		.await
		.unwrap();

	// Mine blocks to confirm the channel and wait for servers to sync
	mine_and_sync(bitcoind, &[server_a, server_b], 6).await;

	// Wait for channel to become usable (mines blocks periodically to trigger chain sync)
	wait_for_usable_channel(server_a.client(), bitcoind, Duration::from_secs(60)).await;

	open_resp.user_channel_id
}

/// Wait for exactly `count` channels, all usable, without mining additional blocks.
/// Use zero to wait until no channels remain.
pub async fn wait_for_channels(
	server: &LdkServerHandle, count: usize, timeout: Duration,
) -> Vec<Channel> {
	let start = Instant::now();
	loop {
		let channels =
			server.client().list_channels(ListChannelsRequest {}).await.unwrap().channels;
		if channels.len() == count && channels.iter().all(|c| c.is_usable) {
			return channels;
		}
		assert!(start.elapsed() < timeout, "Waiting for {count} usable channels: {channels:?}");
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
}

/// Initiate a cooperative close, retrying transient Lightning errors for up to five seconds.
pub async fn close_channel(
	initiator: &LdkServerHandle, peer: &LdkServerHandle, user_channel_id: &str,
) {
	const RETRY_TIMEOUT: Duration = Duration::from_secs(5);
	let start = Instant::now();
	let mut logged_error = false;
	loop {
		let result = initiator
			.client()
			.close_channel(CloseChannelRequest {
				user_channel_id: user_channel_id.to_string(),
				counterparty_node_id: peer.node_id().to_string(),
			})
			.await;
		match result {
			Ok(_) => return,
			Err(error) => {
				if !logged_error {
					eprintln!("Failed to close channel {user_channel_id}: {error:?}");
					logged_error = true;
				}
				// The last HTLC's asynchronous monitor update can briefly block shutdown,
				// even after PaymentSuccessful and PaymentForwarded have been emitted.
				assert_eq!(error.error_code, LdkServerErrorCode::LightningError);
				assert!(start.elapsed() < RETRY_TIMEOUT, "Channel closure failed: {error:?}");
				tokio::time::sleep(Duration::from_millis(200)).await;
			},
		}
	}
}

/// Send a BOLT11 payment, wait for both peers to record success, and return the sender's payment ID.
pub async fn send_bolt11_payment(
	sender: &LdkServerHandle, receiver: &LdkServerHandle, amount_msat: u64,
) -> String {
	let mut sent = sender.client().subscribe_events().await.unwrap();
	let mut received = receiver.client().subscribe_events().await.unwrap();
	let invoice = receiver
		.client()
		.bolt11_receive(Bolt11ReceiveRequest {
			amount_msat: Some(amount_msat),
			description: None,
			expiry_secs: 3600,
		})
		.await
		.unwrap();
	let payment_id = sender
		.client()
		.bolt11_send(Bolt11SendRequest {
			invoice: invoice.invoice,
			amount_msat: None,
			route_parameters: None,
		})
		.await
		.unwrap()
		.payment_id;
	let sent = wait_for_event(&mut sent, |event| {
		matches!(event, Event::PaymentSuccessful(e) if e.payment.as_ref().is_some_and(|p| p.payment_id == payment_id))
	})
	.await;
	let Some(Event::PaymentSuccessful(sent)) = sent.event else {
		panic!("Expected a PaymentSuccessful event after paying the BOLT11 invoice");
	};
	let received =
		wait_for_event(&mut received, |event| matches!(event, Event::PaymentReceived(_))).await;
	let Some(Event::PaymentReceived(received)) = received.event else {
		panic!("Expected a PaymentReceived event after paying the BOLT11 invoice");
	};
	// Events include the stored payment records. IDs are local to each node, so correlate by hash.
	for (id, payment) in [(sent.payment_id, sent.payment), (received.payment_id, received.payment)]
	{
		let payment = payment.unwrap();
		assert_eq!(payment.payment_id, id);
		assert_eq!(payment.status, PaymentStatus::Succeeded as i32);
		assert_eq!(payment.amount_msat, Some(amount_msat));
		let Some(payment_kind::Kind::Bolt11(details)) = payment.kind.unwrap().kind else {
			panic!("Expected a BOLT11 payment");
		};
		assert_eq!(details.hash, invoice.payment_hash);
	}
	payment_id
}

/// List payments for a test server, asserting that the history fits on one page.
pub async fn list_payments(server: &LdkServerHandle) -> Vec<Payment> {
	let response =
		server.client().list_payments(ListPaymentsRequest { page_token: None }).await.unwrap();
	assert!(response.next_page_token.is_none());
	response.payments
}

/// Wait for the expected spendable onchain balance and for Lightning funds to settle.
pub async fn wait_for_settled_balance(
	server: &LdkServerHandle, expected_sats: u64, timeout: Duration,
) -> GetBalancesResponse {
	let start = Instant::now();
	loop {
		let balances = server.client().get_balances(GetBalancesRequest {}).await.unwrap();
		if balances.total_onchain_balance_sats == expected_sats
			&& balances.spendable_onchain_balance_sats == expected_sats
			&& balances.total_anchor_channels_reserve_sats == 0
			&& balances.total_lightning_balance_sats == 0
			&& balances.lightning_balances.is_empty()
			&& balances.pending_balances_from_channel_closures.iter().all(|balance| {
				matches!(
					balance.balance_type,
					Some(pending_sweep_balance::BalanceType::AwaitingThresholdConfirmations(_))
				)
			}) {
			return balances;
		}
		assert!(
			start.elapsed() < timeout,
			"Expected {} to settle at {expected_sats} sats onchain: {balances:?}",
			server.node_id()
		);
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
}

/// Wait for exactly `count` announced channels with both routing directions enabled.
pub async fn wait_for_gossip(server: &LdkServerHandle, count: usize, timeout: Duration) {
	let start = Instant::now();
	loop {
		let graph = server.client().graph_list_channels(GraphListChannelsRequest {}).await.unwrap();
		let mut ready = graph.short_channel_ids.len() == count;
		for short_channel_id in graph.short_channel_ids {
			if !ready {
				break;
			}
			let channel = server
				.client()
				.graph_get_channel(GraphGetChannelRequest { short_channel_id })
				.await
				.unwrap()
				.channel
				.unwrap();
			ready = channel.one_to_two.is_some_and(|update| update.enabled)
				&& channel.two_to_one.is_some_and(|update| update.enabled);
		}
		if ready {
			return;
		}
		assert!(
			start.elapsed() < timeout,
			"Timed out waiting for {count} channel announcements with enabled routing updates"
		);
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
}

/// Wait for exactly `count` forwarded payments, asserting that the history fits on one page.
pub async fn wait_for_forwarded_payments(
	server: &LdkServerHandle, count: usize, timeout: Duration,
) -> Vec<ForwardedPayment> {
	let start = Instant::now();
	loop {
		let response = server
			.client()
			.list_forwarded_payments(ListForwardedPaymentsRequest { page_token: None })
			.await
			.unwrap();
		assert!(response.next_page_token.is_none());
		if response.forwarded_payments.len() == count {
			return response.forwarded_payments;
		}
		assert!(start.elapsed() < timeout, "Expected {count} forwards: {response:?}");
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
}

/// Mine until each server has exactly one confirmed force-close claim with the expected source.
/// Returns the claims in server order, before the channel monitors hand them to the wallet.
pub async fn wait_for_force_close_claims(
	bitcoind: &TestBitcoind, servers: &[(&LdkServerHandle, BalanceSource)], timeout: Duration,
) -> Vec<ClaimableAwaitingConfirmations> {
	let handles: Vec<_> = servers.iter().map(|(server, _)| *server).collect();
	let start = Instant::now();
	loop {
		mine_and_sync(bitcoind, &handles, 1).await;
		let mut balances = Vec::new();
		for (server, _) in servers {
			balances.push(server.client().get_balances(GetBalancesRequest {}).await.unwrap());
		}
		let claims: Option<Vec<_>> = balances
			.iter()
			.zip(servers)
			.map(|(balances, (_, source))| match balances.lightning_balances.as_slice() {
				[LightningBalance {
					balance_type:
						Some(lightning_balance::BalanceType::ClaimableAwaitingConfirmations(claim)),
				}] if claim.source == *source as i32 => Some(claim.clone()),
				_ => None,
			})
			.collect();
		if let Some(claims) = claims {
			return claims;
		}
		assert!(start.elapsed() < timeout, "Waiting for confirmed closing outputs: {balances:?}");
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
}

/// Compare funds before and after closure independently of the closing receipts, allowing
/// 5,000 sats for sweep fees and differences from the commitment fee already deducted from
/// Lightning balances. A cheaper cooperative close can slightly increase the onchain balance.
pub fn assert_recovered_balance(before: &GetBalancesResponse, after: &GetBalancesResponse) {
	let before_sats = before.total_onchain_balance_sats + before.total_lightning_balance_sats;
	let after_sats = after.total_onchain_balance_sats;
	let fee_allowance_sats = 5_000;
	assert!(
		before_sats.abs_diff(after_sats) <= fee_allowance_sats,
		"Expected recovery of {before_sats} sats within {fee_allowance_sats} sats for fees, got {after_sats}"
	);
}

/// Mine until `expected_receipts` new onchain receipts and all new onchain payments confirm,
/// then reconcile them with the starting balance. Incoming amounts are already net of fees;
/// outgoing amounts exclude their separately reported fees. Payment histories must fit on one page.
pub async fn expected_onchain_balance(
	bitcoind: &TestBitcoind, server: &LdkServerHandle, balance_before: u64,
	payments_before: &[Payment], expected_receipts: usize, timeout: Duration,
) -> u64 {
	let start = Instant::now();
	loop {
		let new_payments: Vec<_> = list_payments(server)
			.await
			.into_iter()
			.filter(|payment| {
				matches!(
					payment.kind.as_ref().and_then(|kind| kind.kind.as_ref()),
					Some(payment_kind::Kind::Onchain(_))
				) && !payments_before.iter().any(|old| old.payment_id == payment.payment_id)
			})
			.collect();
		let receipts = new_payments
			.iter()
			.filter(|payment| payment.direction == PaymentDirection::Inbound as i32)
			.count();
		if receipts == expected_receipts
			&& new_payments.iter().all(|payment| payment.status == PaymentStatus::Succeeded as i32)
		{
			let mut expected_msat = i128::from(balance_before) * 1000;
			for payment in new_payments {
				let amount =
					i128::from(payment.amount_msat.expect("Missing onchain payment amount"));
				let direction = PaymentDirection::from_i32(payment.direction)
					.expect("Unexpected onchain payment direction");
				match direction {
					PaymentDirection::Inbound => expected_msat += amount,
					PaymentDirection::Outbound => {
						let fee = payment.fee_paid_msat.expect("Missing onchain transaction fee");
						expected_msat -= amount + i128::from(fee);
					},
				}
			}
			assert_eq!(expected_msat % 1000, 0);
			return u64::try_from(expected_msat / 1000).unwrap();
		}
		assert!(
			start.elapsed() < timeout,
			"Waiting for {expected_receipts} confirmed onchain payments: {new_payments:?}"
		);
		// Broadcast and wallet sync are asynchronous, so keep confirming until the public API
		// reports the closing/sweep payments as succeeded.
		mine_and_sync(bitcoind, &[server], 1).await;
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
}
