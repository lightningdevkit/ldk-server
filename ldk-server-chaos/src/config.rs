#[derive(Clone)]
pub struct NodeConfig {
	#[allow(dead_code)]
	pub node_index: usize,
	pub listening_address: String,
	pub rest_address: String,
	pub alias: String,
	pub storage_dir: String,
	pub bitcoind_rpc_address: String,
	pub bitcoind_rpc_user: String,
	pub bitcoind_rpc_password: String,
}

impl NodeConfig {
	pub fn new(
		node_index: usize, storage_dir: String, bitcoind_rpc_url: String,
		bitcoind_rpc_user: String, bitcoind_rpc_password: String,
	) -> Self {
		// Parse bitcoind URL to get host:port
		let bitcoind_rpc_address = bitcoind_rpc_url
			.strip_prefix("http://")
			.or_else(|| bitcoind_rpc_url.strip_prefix("https://"))
			.unwrap_or(&bitcoind_rpc_url)
			.to_string();

		Self {
			node_index,
			listening_address: format!("localhost:{}", 9700 + node_index),
			rest_address: format!("127.0.0.1:{}", 3100 + node_index),
			alias: format!("ChaosNode{}", node_index),
			storage_dir,
			bitcoind_rpc_address,
			bitcoind_rpc_user,
			bitcoind_rpc_password,
		}
	}

	pub fn to_toml(&self) -> String {
		format!(
			r#"[node]
network = "regtest"
listening_addresses = ["{listening_address}"]
rest_service_address = "{rest_address}"
alias = "{alias}"

[storage.disk]
dir_path = "{storage_dir}"

[bitcoind]
rpc_address = "{bitcoind_rpc_address}"
rpc_user = "{bitcoind_rpc_user}"
rpc_password = "{bitcoind_rpc_password}"

[log]
level = "Trace"
file = "{log_file}"
"#,
			listening_address = self.listening_address,
			rest_address = self.rest_address,
			alias = self.alias,
			storage_dir = self.storage_dir,
			log_file = format!("{}/ldk-server.log", self.storage_dir),
			bitcoind_rpc_address = self.bitcoind_rpc_address,
			bitcoind_rpc_user = self.bitcoind_rpc_user,
			bitcoind_rpc_password = self.bitcoind_rpc_password,
		)
	}
}
