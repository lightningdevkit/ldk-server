// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use corepc_node::Node;
use hex_conservative::DisplayHex;
use ldk_server_client::client::LdkServerClient;
use ldk_server_client::ldk_server_protos::api::{GetNodeInfoRequest, GetNodeInfoResponse};
use ldk_server_protos::api::{
	GetBalancesRequest, ListChannelsRequest, OnchainReceiveRequest, OpenChannelRequest,
};

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
	pub rest_port: u16,
	pub p2p_port: u16,
	pub storage_dir: PathBuf,
	pub api_key: String,
	pub tls_cert_path: PathBuf,
	pub node_id: String,
	pub exchange_name: String,
	client: LdkServerClient,
}

impl LdkServerHandle {
	/// Starts a new ldk-server instance against the given bitcoind.
	/// Waits until the server is ready to accept requests.
	pub async fn start(bitcoind: &TestBitcoind) -> Self {
		#[allow(deprecated)]
		let storage_dir = tempfile::tempdir().unwrap().into_path();
		let rest_port = find_available_port();
		let p2p_port = find_available_port();

		let (rpc_host, rpc_port_num, rpc_user, rpc_password) = bitcoind.rpc_details();
		let rpc_address = format!("{rpc_host}:{rpc_port_num}");

		let exchange_name = format!("e2e_test_exchange_{rest_port}");

		let config_content = format!(
			r#"[node]
network = "regtest"
listening_addresses = ["127.0.0.1:{p2p_port}"]
rest_service_address = "127.0.0.1:{rest_port}"
alias = "e2e-test-node"

[storage.disk]
dir_path = "{storage_dir}"

[bitcoind]
rpc_address = "{rpc_address}"
rpc_user = "{rpc_user}"
rpc_password = "{rpc_password}"

[rabbitmq]
connection_string = "amqp://guest:guest@localhost:5672/%2f"
exchange_name = "{exchange_name}"

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
"#,
			storage_dir = storage_dir.display(),
		);

		let config_path = storage_dir.join("config.toml");
		std::fs::write(&config_path, &config_content).unwrap();

		let server_binary = server_binary_path();
		let mut child = Command::new(&server_binary)
			.arg(config_path.to_str().unwrap())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn()
			.unwrap_or_else(|e| {
				panic!("Failed to start ldk-server binary at {:?}: {}", server_binary, e)
			});

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

		let base_url = format!("127.0.0.1:{rest_port}");
		let client = LdkServerClient::new(base_url, api_key.clone(), &tls_cert_pem).unwrap();

		let mut handle = Self {
			child: Some(child),
			rest_port,
			p2p_port,
			storage_dir,
			api_key,
			tls_cert_path,
			node_id: String::new(),
			exchange_name,
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
		format!("127.0.0.1:{}", self.rest_port)
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

/// Run a CLI command against the given server handle and return parsed JSON output.
pub fn run_cli(handle: &LdkServerHandle, args: &[&str]) -> serde_json::Value {
	let stdout = run_cli_raw(handle, args);
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
		})
		.await
		.unwrap();

	// Mine blocks to confirm the channel and wait for servers to sync
	mine_and_sync(bitcoind, &[server_a, server_b], 6).await;

	// Wait for channel to become usable (mines blocks periodically to trigger chain sync)
	wait_for_usable_channel(server_a.client(), bitcoind, Duration::from_secs(60)).await;

	open_resp.user_channel_id
}

/// RabbitMQ event consumer for verifying events published by ldk-server.
pub struct RabbitMqEventConsumer {
	_connection: lapin::Connection,
	channel: lapin::Channel,
	queue_name: String,
}

impl RabbitMqEventConsumer {
	/// Connect to RabbitMQ and create an exclusive queue bound to the given exchange.
	pub async fn new(exchange_name: &str) -> Self {
		use lapin::options::{ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions};
		use lapin::types::FieldTable;
		use lapin::{ConnectionProperties, ExchangeKind};

		let connection = lapin::Connection::connect(
			"amqp://guest:guest@localhost:5672/%2f",
			ConnectionProperties::default(),
		)
		.await
		.expect("Failed to connect to RabbitMQ");

		let channel = connection.create_channel().await.expect("Failed to create channel");

		// Declare exchange (idempotent — may already exist from the server)
		channel
			.exchange_declare(
				exchange_name,
				ExchangeKind::Fanout,
				ExchangeDeclareOptions { durable: true, ..Default::default() },
				FieldTable::default(),
			)
			.await
			.expect("Failed to declare exchange");

		// Create exclusive auto-delete queue with server-generated name
		let queue = channel
			.queue_declare(
				"",
				QueueDeclareOptions { exclusive: true, auto_delete: true, ..Default::default() },
				FieldTable::default(),
			)
			.await
			.expect("Failed to declare queue");
		let queue_name = queue.name().to_string();

		channel
			.queue_bind(
				&queue_name,
				exchange_name,
				"",
				QueueBindOptions::default(),
				FieldTable::default(),
			)
			.await
			.expect("Failed to bind queue");

		Self { _connection: connection, channel, queue_name }
	}

	/// Consume up to `count` events, waiting up to `timeout` for each.
	pub async fn consume_events(
		&self, count: usize, timeout: Duration,
	) -> Vec<ldk_server_protos::events::EventEnvelope> {
		use futures_util::StreamExt;
		use lapin::options::{BasicAckOptions, BasicConsumeOptions};
		use lapin::types::FieldTable;
		use prost::Message;

		let mut consumer = self
			.channel
			.basic_consume(
				&self.queue_name,
				&format!("consumer_{}", self.queue_name),
				BasicConsumeOptions::default(),
				FieldTable::default(),
			)
			.await
			.expect("Failed to start consumer");

		let mut events = Vec::new();
		for _ in 0..count {
			match tokio::time::timeout(timeout, consumer.next()).await {
				Ok(Some(Ok(delivery))) => {
					let event = ldk_server_protos::events::EventEnvelope::decode(&*delivery.data)
						.expect("Failed to decode event");
					self.channel
						.basic_ack(delivery.delivery_tag, BasicAckOptions::default())
						.await
						.expect("Failed to ack");
					events.push(event);
				},
				_ => break,
			}
		}
		events
	}
}
