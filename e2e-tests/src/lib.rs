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
use std::time::Duration;

use corepc_node::Node;
use hex_conservative::DisplayHex;
use ldk_server_client::client::LdkServerClient;
use ldk_server_client::ldk_server_grpc::api::{GetNodeInfoRequest, GetNodeInfoResponse};
use ldk_server_grpc::api::{
	GetBalancesRequest, ListChannelsRequest, OnchainReceiveRequest, OpenChannelRequest,
};
use serde_json::Value;

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
		let bitcoind = match std::env::var("BITCOIND_EXE") {
			Ok(path) => Node::new(path).unwrap(),
			Err(_) => Node::from_downloaded().unwrap(),
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
	pub api_key: String,
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
	Bitcoind { rpc_address: String, rpc_user: String, rpc_password: String },
	Electrum { server_url: String },
	Esplora { server_url: String },
}

impl ChainSource {
	/// Render the chain source as its TOML section.
	fn to_toml(&self) -> String {
		match self {
			ChainSource::Bitcoind { rpc_address, rpc_user, rpc_password } => format!(
				"[bitcoind]\nrpc_address = \"{}\"\nrpc_user = \"{}\"\nrpc_password = \"{}\"",
				rpc_address, rpc_user, rpc_password
			),
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
	chain_source: ChainSource,
	metrics_auth: Option<(String, String)>,
	log: Option<(Option<String>, String)>,
	tls_hosts: Option<Vec<String>>,
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
			chain_source: ChainSource::Bitcoind {
				rpc_address: params.rpc_address.clone(),
				rpc_user: params.rpc_user.clone(),
				rpc_password: params.rpc_password.clone(),
			},
			metrics_auth: None,
			log: None,
			tls_hosts: None,
		}
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
		let TestServerParams { grpc_port, p2p_port, storage_dir, .. } = params;

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

		// Wait for the api_key and tls.crt files to appear in the network subdir
		let network_dir = storage_dir.join("regtest");
		let api_key_path = network_dir.join("api_key");
		let tls_cert_path = storage_dir.join("tls.crt");

		wait_for_file(&api_key_path, Duration::from_secs(30)).await;
		wait_for_file(&tls_cert_path, Duration::from_secs(30)).await;

		// Read the API key (raw bytes -> hex)
		let api_key_bytes = std::fs::read(&api_key_path).unwrap();
		let api_key = api_key_bytes.to_lower_hex_string();

		// Read TLS cert
		let tls_cert_pem = std::fs::read(&tls_cert_path).unwrap();

		let base_url = format!("127.0.0.1:{grpc_port}");
		let client = LdkServerClient::new(base_url, api_key.clone(), &tls_cert_pem).unwrap();

		let mut handle = Self {
			child: Some(child),
			grpc_port,
			p2p_port,
			storage_dir,
			config_path,
			api_key,
			tls_cert_path,
			node_id: String::new(),
			client,
		};

		// Wait for server to be ready and get node info
		let node_info = wait_for_server_ready(&handle, Duration::from_secs(60)).await;
		handle.node_id = node_info.node_id;

		handle
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

	let server_binary = server_binary_path();
	let child = Command::new(&server_binary)
		.arg(config_path.to_str().unwrap())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.unwrap_or_else(|e| {
			panic!("Failed to start ldk-server binary at {:?}: {}", server_binary, e)
		});

	(child, params, config_path)
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
		let mcp_path = mcp_binary_path();
		let mut child = Command::new(&mcp_path)
			.env("LDK_BASE_URL", server.base_url())
			.env("LDK_API_KEY", &server.api_key)
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
		.arg("--api-key")
		.arg(&handle.api_key)
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
			channel_amount_sats,
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
