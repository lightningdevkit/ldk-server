use std::env;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

macro_rules! tprintln {
	($($arg:tt)*) => {
		println!("[{}] {}", chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true), format!($($arg)*))
	};
}

use ldk_server_client::client::LdkServerClient;
use ldk_server_protos::api::{
	Bolt11ReceiveRequest, Bolt11SendRequest, ConnectPeerRequest, GetBalancesRequest,
	GetNodeInfoRequest, ListChannelsRequest, OnchainReceiveRequest, OpenChannelRequest,
};
use ldk_server_protos::types::bolt11_invoice_description::Kind as DescriptionKind;
use ldk_server_protos::types::Bolt11InvoiceDescription;
use rand::{Rng, SeedableRng};
use tokio::sync::Mutex;
use tokio::time::sleep;

mod bitcoind;
mod config;

use bitcoind::BitcoindClient;
use config::NodeConfig;

const NUM_NODES: usize = 3;
const NUM_CHANNELS: usize = 100;
const CHANNEL_AMOUNT_SATS: u64 = 100_000;
const PAYMENT_AMOUNT_MSAT: u64 = 1;
const PAYMENT_TIMEOUT_SECS: u64 = 60;

/// Tracks payment statistics and timeout detection.
/// Uses a flat array indexed by sender * NUM_NODES + receiver for per-direction tracking.
struct PaymentTracker {
	last_success: [AtomicU64; NUM_NODES * NUM_NODES],
	total_success: AtomicU64,
	total_attempts: AtomicU64,
	start_time: Instant,
}

impl PaymentTracker {
	fn new() -> Self {
		Self {
			last_success: std::array::from_fn(|_| AtomicU64::new(0)),
			total_success: AtomicU64::new(0),
			total_attempts: AtomicU64::new(0),
			start_time: Instant::now(),
		}
	}

	fn elapsed_millis(&self) -> u64 {
		self.start_time.elapsed().as_millis() as u64
	}

	fn record_attempt(&self) -> u64 {
		self.total_attempts.fetch_add(1, Ordering::Relaxed) + 1
	}

	fn record_success(&self, sender: usize, receiver: usize) -> u64 {
		let idx = sender * NUM_NODES + receiver;
		self.last_success[idx].store(self.elapsed_millis(), Ordering::Relaxed);
		self.total_success.fetch_add(1, Ordering::Relaxed) + 1
	}

	fn get_counts(&self) -> (u64, u64) {
		(self.total_success.load(Ordering::Relaxed), self.total_attempts.load(Ordering::Relaxed))
	}

	/// Returns the successful payments per second rate since start.
	fn get_success_rate(&self) -> f64 {
		let elapsed_secs = self.elapsed_millis() as f64 / 1000.0;
		if elapsed_secs < 0.001 {
			return 0.0;
		}
		let total_success = self.total_success.load(Ordering::Relaxed) as f64;
		total_success / elapsed_secs
	}

	/// Returns Some(direction_str) if a direction has timed out, None otherwise.
	fn check_timeout(&self) -> Option<String> {
		let now = self.elapsed_millis();
		let timeout_millis = PAYMENT_TIMEOUT_SECS * 1000;

		for sender in 0..NUM_NODES {
			for receiver in 0..NUM_NODES {
				if sender == receiver {
					continue;
				}
				let idx = sender * NUM_NODES + receiver;
				let last = self.last_success[idx].load(Ordering::Relaxed);
				// Only check timeout if we've had at least one success in that direction
				if last > 0 && now - last > timeout_millis {
					return Some(format!("{}->{}", sender, receiver));
				}
			}
		}
		None
	}
}

struct NodeHandle {
	config: NodeConfig,
	process: Option<Child>,
	client: Option<LdkServerClient>,
	data_dir: PathBuf,
}

impl NodeHandle {
	fn new(config: NodeConfig, data_dir: PathBuf) -> Self {
		Self { config, process: None, client: None, data_dir }
	}

	async fn start(&mut self) -> anyhow::Result<()> {
		// Write config file
		let config_path = self.data_dir.join("config.toml");
		std::fs::write(&config_path, self.config.to_toml())?;

		// Start ldk-server process (logs go to configured log file, suppress stdout/stderr)
		let process = Command::new("cargo")
			.args(["run", "--bin", "ldk-server", "--"])
			.arg(&config_path)
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()?;

		self.process = Some(process);

		// Wait for server to start and TLS cert to be generated
		let cert_path = self.data_dir.join("tls.crt");
		let log_path = self.data_dir.join("ldk-server.log");
		for i in 0..60 {
			if cert_path.exists() {
				break;
			}
			// Check if process died
			if let Some(ref mut proc) = self.process {
				if let Ok(Some(status)) = proc.try_wait() {
					// Process exited, read log file for errors
					let log_output = std::fs::read_to_string(&log_path).unwrap_or_default();
					anyhow::bail!(
						"ldk-server exited with status {} after {}s. Log:\n{}",
						status,
						i / 2,
						log_output
					);
				}
			}
			sleep(Duration::from_millis(500)).await;
		}

		if !cert_path.exists() {
			// Read log file for errors
			let log_output = std::fs::read_to_string(&log_path).unwrap_or_default();
			if let Some(ref mut proc) = self.process {
				let _ = proc.kill();
			}
			anyhow::bail!(
				"TLS certificate not generated after 30s - ldk-server may have failed to start. Log:\n{}",
				log_output
			);
		}

		// Small additional delay for server to be ready
		sleep(Duration::from_secs(2)).await;

		// Create client - read API key from file (server generates it in regtest/api_key)
		let cert_pem = std::fs::read(&cert_path)?;
		let api_key_path = self.data_dir.join("regtest").join("api_key");
		let api_key_bytes = std::fs::read(&api_key_path)?;
		let api_key = hex::encode(&api_key_bytes);
		let client = LdkServerClient::new(self.config.rest_address.clone(), api_key, &cert_pem)
			.map_err(|e| anyhow::anyhow!(e))?;

		self.client = Some(client);
		Ok(())
	}

	fn kill(&mut self) {
		if let Some(mut process) = self.process.take() {
			// Hard kill - SIGKILL, no graceful shutdown
			let _ = process.kill();
			let _ = process.wait();
		}
		self.client = None;
	}

	fn client(&self) -> Option<&LdkServerClient> {
		self.client.as_ref()
	}
}

impl Drop for NodeHandle {
	fn drop(&mut self) {
		self.kill();
	}
}

/// Kill all ldk-server processes. Called on exit/panic/ctrl-c.
fn kill_all_ldk_servers() {
	let _ = Command::new("pkill").args(["-9", "-f", "ldk-server"]).status();
}

/// Check if a port is available by attempting to bind to it.
fn is_port_available(port: u16) -> bool {
	TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// Check that all required ports are available before starting the test.
/// Returns an error with details about which ports are in use.
fn check_ports_available() -> anyhow::Result<()> {
	let mut ports_in_use = Vec::new();

	for i in 0..NUM_NODES {
		let lightning_port = 9700 + i as u16;
		let rest_port = 3100 + i as u16;

		if !is_port_available(lightning_port) {
			ports_in_use.push(format!("{} (node {} lightning)", lightning_port, i));
		}
		if !is_port_available(rest_port) {
			ports_in_use.push(format!("{} (node {} REST API)", rest_port, i));
		}
	}

	if !ports_in_use.is_empty() {
		anyhow::bail!(
			"The following ports are already in use:\n  {}\n\nPlease stop the processes using these ports before running the chaos test.\nYou can find the processes with: lsof -i :<port>",
			ports_in_use.join("\n  ")
		);
	}

	Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	kill_all_ldk_servers();

	// Check that all required ports are available before proceeding
	check_ports_available()?;

	// Register panic hook to kill ldk-server processes on panic/assert failure
	let default_hook = std::panic::take_hook();
	std::panic::set_hook(Box::new(move |info| {
		kill_all_ldk_servers();
		default_hook(info);
	}));

	tprintln!("=== LDK Server Chaos Test ===\n");

	// Use a fixed data directory under ldk-server-chaos for persistence and easy access
	let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
	// Clean up any previous run
	if data_dir.exists() {
		std::fs::remove_dir_all(&data_dir)?;
	}
	std::fs::create_dir_all(&data_dir)?;

	tprintln!("Data directory: {}", data_dir.display());
	tprintln!("Node logs will be at:");
	for i in 0..NUM_NODES {
		tprintln!("  Node {}: {}", i, data_dir.join(format!("node{}/ldk-server.log", i)).display());
	}

	// Connect to bitcoind
	let bitcoind = BitcoindClient::new_default()?;
	tprintln!("Connected to bitcoind");

	// Ensure we have enough blocks
	let info = bitcoind.get_blockchain_info()?;
	if info.blocks < 101 {
		tprintln!("Mining initial blocks...");
		bitcoind.mine_blocks(101 - info.blocks as u32)?;
	}

	// Create and start nodes
	let mut nodes: Vec<Arc<Mutex<NodeHandle>>> = Vec::new();

	for i in 0..NUM_NODES {
		let node_dir = data_dir.join(format!("node{}", i));
		std::fs::create_dir_all(&node_dir)?;

		let config = NodeConfig::new(
			i,
			node_dir.to_str().unwrap().to_string(),
			bitcoind.rpc_url.clone(),
			bitcoind.rpc_user.clone(),
			bitcoind.rpc_password.clone(),
		);

		let mut handle = NodeHandle::new(config, node_dir);
		tprintln!("Starting node {}...", i);
		handle.start().await?;
		tprintln!("Node {} started (REST: {})", i, handle.config.rest_address);

		nodes.push(Arc::new(Mutex::new(handle)));
	}

	// Get node info and fund nodes by mining to their addresses
	let mut node_ids = Vec::new();
	let mut node_addresses = Vec::new();

	for (i, node) in nodes.iter().enumerate() {
		let node = node.lock().await;
		let client = node.client().expect("Node not started");
		let info = client.get_node_info(GetNodeInfoRequest {}).await?;
		node_ids.push(info.node_id.clone());
		node_addresses.push(node.config.listening_address.clone());
		tprintln!("Node {} pubkey: {}", i, info.node_id);

		// Get funding address and mine blocks to it
		let addr_resp = client.onchain_receive(OnchainReceiveRequest {}).await?;
		tprintln!("Node {} funding address: {}", i, addr_resp.address);

		// Fund by mining blocks directly to the node's address
		bitcoind.mine_to_address(20, &addr_resp.address)?;
		tprintln!("Node {} funded with 20 block rewards", i);
	}

	// Mine 101 blocks to mature the coinbase outputs (100 block requirement)
	tprintln!("\nMining 101 blocks to mature coinbase outputs...");
	bitcoind.mine_blocks(101)?;

	// Restart nodes with 0-conf trust for all other nodes
	tprintln!("\nRestarting nodes with 0-conf trust configuration...");
	for (i, node) in nodes.iter().enumerate() {
		let mut node = node.lock().await;
		node.kill();

		// Configure to trust all other nodes for 0-conf
		let trusted_peers: Vec<String> = node_ids
			.iter()
			.enumerate()
			.filter(|(j, _)| *j != i)
			.map(|(_, id)| id.clone())
			.collect();
		node.config.trusted_peers_0conf = trusted_peers;

		node.start().await?;
		tprintln!("Node {} restarted with 0-conf trust", i);
	}

	// Wait for nodes to have spendable balance
	tprintln!("Waiting for nodes to sync and have spendable balance...");
	for (i, node) in nodes.iter().enumerate() {
		loop {
			let node = node.lock().await;
			let client = node.client().expect("Node not started");
			let balances = client.get_balances(GetBalancesRequest {}).await?;
			if balances.spendable_onchain_balance_sats > 0 {
				tprintln!(
					"Node {} has spendable balance: {} sats",
					i,
					balances.spendable_onchain_balance_sats
				);
				break;
			}
			drop(node);
			sleep(Duration::from_secs(1)).await;
		}
	}

	// Open channels in batches due to LDK's MAX_UNFUNDED_CHANS_PER_PEER limit (currently 4).
	//
	// When a node receives an open_channel message, LDK checks if the sender already has too many
	// "unfunded" inbound channels (channels that haven't completed the funding handshake yet).
	// If so, LDK silently rejects the channel - the opener's open_channel() call succeeds, but
	// the receiver never processes it.
	//
	// 0-conf channels DO bypass this limit once they complete the handshake (minimum_depth == 0
	// means they're not considered "unfunded"), but there's a race condition: the unfunded check
	// happens in the message handler BEFORE the OpenChannelRequest event fires and 0-conf is
	// configured. So if we send 5+ open_channel messages before any complete their handshake,
	// the 5th+ get rejected.
	//
	// Solution: Open in batches of 4, wait for the receiver to see those channels (meaning the
	// handshake completed), then open the next batch.
	const BATCH_SIZE: usize = 4;
	let channel_pairs = [(0, 1), (1, 2)]; // (opener, receiver)

	for (opener_idx, receiver_idx) in channel_pairs {
		tprintln!(
			"\nOpening {} channels from Node {} to Node {}...",
			NUM_CHANNELS,
			opener_idx,
			receiver_idx
		);

		// Track how many channels receiver had before we started (for 1->2, node 1 already has channels)
		let receiver_initial_count = {
			let receiver = nodes[receiver_idx].lock().await;
			let client = receiver.client().expect("Node not started");
			client.list_channels(ListChannelsRequest {}).await?.channels.len()
		};

		for batch in 0..(NUM_CHANNELS + BATCH_SIZE - 1) / BATCH_SIZE {
			let start = batch * BATCH_SIZE;
			let end = (start + BATCH_SIZE).min(NUM_CHANNELS);

			// Open this batch
			{
				let opener = nodes[opener_idx].lock().await;
				let client = opener.client().expect("Node not started");

				for i in start..end {
					match client
						.open_channel(OpenChannelRequest {
							node_pubkey: node_ids[receiver_idx].clone(),
							address: node_addresses[receiver_idx].clone(),
							channel_amount_sats: CHANNEL_AMOUNT_SATS,
							push_to_counterparty_msat: Some((CHANNEL_AMOUNT_SATS / 2) * 1000),
							channel_config: None,
							announce_channel: false,
						})
						.await
					{
						Ok(resp) => {
							if (i + 1) % 10 == 0 || i == 0 {
								tprintln!(
									"Opened channel {}/{} ({}->{}): {}",
									i + 1,
									NUM_CHANNELS,
									opener_idx,
									receiver_idx,
									resp.user_channel_id
								);
							}
						},
						Err(e) => {
							tprintln!(
								"Failed to open channel {}/{} ({}->{}): {}",
								i + 1,
								NUM_CHANNELS,
								opener_idx,
								receiver_idx,
								e
							);
						},
					}
				}
			} // Release opener lock

			// Wait for receiver to see these channels before opening more
			if end < NUM_CHANNELS {
				let target_count = receiver_initial_count + end;
				loop {
					let receiver = nodes[receiver_idx].lock().await;
					let client = receiver.client().expect("Node not started");
					let channels = client.list_channels(ListChannelsRequest {}).await?;
					if channels.channels.len() >= target_count {
						break;
					}
					drop(receiver);
					sleep(Duration::from_millis(50)).await;
				}
			}
		}
		tprintln!(
			"Successfully opened {} channels ({}->{})",
			NUM_CHANNELS,
			opener_idx,
			receiver_idx
		);
	}

	// Wait for all channels to become visible and usable on each node
	// Node 0: 100 channels (all with Node 1)
	// Node 1: 200 channels (100 with Node 0, 100 with Node 2)
	// Node 2: 100 channels (all with Node 1)
	let expected_counts = [NUM_CHANNELS, NUM_CHANNELS * 2, NUM_CHANNELS];
	tprintln!("\nWaiting for channels to become visible and usable...");
	for (i, node) in nodes.iter().enumerate() {
		let mut last_count = 0;
		let mut last_usable_count = 0;
		loop {
			let node = node.lock().await;
			let client = node.client().expect("Node not started");
			let channels = client.list_channels(ListChannelsRequest {}).await?;
			let total_count = channels.channels.len();
			let usable_count = channels.channels.iter().filter(|ch| ch.is_usable).count();

			if total_count != last_count || usable_count != last_usable_count {
				tprintln!(
					"Node {} channels: {}/{} total, {}/{} usable",
					i,
					total_count,
					expected_counts[i],
					usable_count,
					expected_counts[i]
				);
				last_count = total_count;
				last_usable_count = usable_count;
			}

			if usable_count >= expected_counts[i] {
				tprintln!("Node {} ready: {} channels usable", i, usable_count);
				break;
			}
			drop(node);
			sleep(Duration::from_millis(500)).await;
		}
	}

	// Start payment loop and chaos monkeys
	tprintln!("\n=== Starting payment loops and chaos monkeys ===\n");

	// Payment tracker for timeout detection
	let payment_tracker = Arc::new(PaymentTracker::new());

	// Spawn a task that hard-exits on Ctrl+C (no graceful shutdown)
	tokio::spawn(async {
		let _ = tokio::signal::ctrl_c().await;
		tprintln!("\nReceived Ctrl+C, killing ldk-server processes...");
		kill_all_ldk_servers();
		std::process::exit(0);
	});

	let chaos_enabled = !env::args().any(|arg| arg == "--no-chaos");

	// Start 20 parallel payment loops
	for loop_id in 0..20 {
		let nodes_clone = nodes.clone();
		let tracker_clone = payment_tracker.clone();
		tokio::spawn(async move {
			payment_loop(nodes_clone, loop_id, tracker_clone).await;
		});
	}

	// Start independent chaos monkey for each node (unless disabled)
	let mut chaos_handles = Vec::new();
	if chaos_enabled {
		for node_idx in 0..NUM_NODES {
			let nodes_clone = nodes.clone();
			let node_ids_clone = node_ids.clone();
			let node_addresses_clone = node_addresses.clone();
			let handle = tokio::spawn(async move {
				chaos_monkey_for_node(nodes_clone, node_ids_clone, node_addresses_clone, node_idx)
					.await
			});
			chaos_handles.push(handle);
		}
	} else {
		tprintln!("Chaos monkey disabled (--no-chaos)");
	}

	// Timeout monitor task
	let tracker_clone = payment_tracker.clone();
	let timeout_handle = tokio::spawn(async move {
		loop {
			if let Some(direction) = tracker_clone.check_timeout() {
				return Some(direction);
			}
			sleep(Duration::from_millis(500)).await;
		}
	});

	// Periodic metrics reporter (every 10 seconds)
	let tracker_clone = payment_tracker.clone();
	tokio::spawn(async move {
		let mut interval = tokio::time::interval(Duration::from_secs(10));
		interval.tick().await; // Skip immediate first tick
		loop {
			interval.tick().await;
			let (success, attempts) = tracker_clone.get_counts();
			let rate = tracker_clone.get_success_rate();
			let success_pct =
				if attempts > 0 { (success as f64 / attempts as f64) * 100.0 } else { 0.0 };
			tprintln!(
				"[METRICS] Rate: {:.2} payments/sec | Success: {}/{} ({:.1}%)",
				rate,
				success,
				attempts,
				success_pct
			);
		}
	});

	// Wait for chaos monkey exit, payment timeout, or run forever if chaos disabled
	if chaos_enabled {
		tokio::select! {
			_ = futures::future::select_all(chaos_handles) => {
				tprintln!("\nA chaos monkey exited (channel closed?), exiting...");
			}
			result = timeout_handle => {
				if let Ok(Some(direction)) = result {
					tprintln!("\nPAYMENT TIMEOUT: No successful payment in direction {} for {}s, exiting...", direction, PAYMENT_TIMEOUT_SECS);
				}
			}
		}
	} else {
		// Without chaos monkey, just wait for payment timeout
		if let Ok(Some(direction)) = timeout_handle.await {
			tprintln!(
				"\nPAYMENT TIMEOUT: No successful payment in direction {} for {}s, exiting...",
				direction,
				PAYMENT_TIMEOUT_SECS
			);
		}
	}

	// Cleanup
	tprintln!("\nKilling nodes...");
	for node in &nodes {
		node.lock().await.kill();
	}

	tprintln!("Done!");
	Ok(())
}

async fn payment_loop(
	nodes: Vec<Arc<Mutex<NodeHandle>>>, loop_id: usize, tracker: Arc<PaymentTracker>,
) {
	let mut rng = rand::rngs::SmallRng::from_os_rng();

	loop {
		// Pick random sender and receiver (must be different)
		let sender_idx = rng.random_range(0..NUM_NODES);
		let receiver_idx = loop {
			let r = rng.random_range(0..NUM_NODES);
			if r != sender_idx {
				break r;
			}
		};

		// Get invoice from receiver
		let invoice = {
			let receiver = nodes[receiver_idx].lock().await;
			let Some(client) = receiver.client() else {
				tprintln!("[L{}] Node {} down, skipping", loop_id, receiver_idx);
				sleep(Duration::from_secs(1)).await;
				continue;
			};
			match client
				.bolt11_receive(Bolt11ReceiveRequest {
					amount_msat: Some(PAYMENT_AMOUNT_MSAT),
					description: Some(Bolt11InvoiceDescription {
						kind: Some(DescriptionKind::Direct(format!("L{}", loop_id))),
					}),
					expiry_secs: 3600,
				})
				.await
			{
				Ok(resp) => resp.invoice,
				Err(e) => {
					tprintln!("[L{}] Invoice failed: {}", loop_id, e);
					sleep(Duration::from_secs(1)).await;
					continue;
				},
			}
		};

		// Send payment from sender
		{
			let sender = nodes[sender_idx].lock().await;
			let Some(client) = sender.client() else {
				tprintln!("[L{}] Node {} down, skipping", loop_id, sender_idx);
				sleep(Duration::from_secs(1)).await;
				continue;
			};

			let payment_num = tracker.record_attempt();

			match client
				.bolt11_send(Bolt11SendRequest {
					invoice,
					amount_msat: None,
					route_parameters: None,
				})
				.await
			{
				Ok(resp) => {
					let success_count = tracker.record_success(sender_idx, receiver_idx);
					let (_, total) = tracker.get_counts();
					tprintln!(
						"[L{}] {} -> {}: OK ({}) [{}/{}]",
						loop_id,
						sender_idx,
						receiver_idx,
						resp.payment_id,
						success_count,
						total
					);
				},
				Err(e) => {
					let (success, _) = tracker.get_counts();
					tprintln!(
						"[L{}] {} -> {}: FAIL ({}) [{}/{}]",
						loop_id,
						sender_idx,
						receiver_idx,
						e,
						success,
						payment_num
					);
				},
			}
		}
	}
}

async fn chaos_monkey_for_node(
	nodes: Vec<Arc<Mutex<NodeHandle>>>, node_ids: Vec<String>, node_addresses: Vec<String>,
	node_idx: usize,
) {
	// Use a Send-safe RNG with unique seed per node
	let mut rng = rand::rngs::SmallRng::from_os_rng();

	loop {
		// Wait random interval.
		let wait_secs = rng.random_range(3..=10);
		tprintln!("[Chaos-{}] Waiting {}s before next action...", node_idx, wait_secs);
		sleep(Duration::from_secs(wait_secs)).await;

		tprintln!("[Chaos-{}] SIGKILL node {}...", node_idx, node_idx);
		{
			let mut node = nodes[node_idx].lock().await;
			node.kill();
		}

		// Restart
		tprintln!("[Chaos-{}] Restarting node {}...", node_idx, node_idx);
		{
			let mut node = nodes[node_idx].lock().await;
			if let Err(e) = node.start().await {
				tprintln!("[Chaos-{}] Failed to restart node {}: {}", node_idx, node_idx, e);
				continue;
			} else {
				tprintln!("[Chaos-{}] Node {} restarted successfully", node_idx, node_idx);
			}
		}

		// Reconnect to all other peers in a loop until all succeed
		tprintln!("[Chaos-{}] Reconnecting node {} to other nodes...", node_idx, node_idx);
		for peer_idx in 0..NUM_NODES {
			if peer_idx == node_idx {
				continue;
			}
			loop {
				let node = nodes[node_idx].lock().await;
				let Some(client) = node.client() else {
					drop(node);
					sleep(Duration::from_millis(500)).await;
					continue;
				};
				match client
					.connect_peer(ConnectPeerRequest {
						node_pubkey: node_ids[peer_idx].clone(),
						address: node_addresses[peer_idx].clone(),
						persist: Some(false), // Don't persist, already persisted from channel open
					})
					.await
				{
					Ok(_) => {
						tprintln!(
							"[Chaos-{}] Node {} reconnected to node {}",
							node_idx,
							node_idx,
							peer_idx
						);
						break; // Success, move to next peer
					},
					Err(e) => {
						tprintln!(
							"[Chaos-{}] Node {} failed to reconnect to {}: {}, retrying...",
							node_idx,
							node_idx,
							peer_idx,
							e
						);
						drop(node);
						sleep(Duration::from_millis(500)).await;
						// Continue retrying
					},
				}
			}
		}
	}
}
