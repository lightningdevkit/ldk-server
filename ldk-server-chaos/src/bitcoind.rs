use std::env;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use corepc_client::bitcoin::Address;
use corepc_client::client_sync::v28::Client;
use corepc_client::client_sync::Auth;

pub struct BitcoindClient {
	client: Client,
	pub rpc_url: String,
	pub rpc_user: String,
	pub rpc_password: String,
}

pub struct BlockchainInfo {
	pub blocks: u64,
}

impl BitcoindClient {
	pub fn new(rpc_url: &str, rpc_user: &str, rpc_password: &str) -> anyhow::Result<Self> {
		let auth = Auth::UserPass(rpc_user.to_string(), rpc_password.to_string());
		let client = Client::new_with_auth(rpc_url, auth.clone())
			.map_err(|e| anyhow::anyhow!("Failed to create bitcoind client: {}", e))?;

		Ok(Self {
			client,
			rpc_url: rpc_url.to_string(),
			rpc_user: rpc_user.to_string(),
			rpc_password: rpc_password.to_string(),
		})
	}

	pub fn new_with_cookie(rpc_url: &str, cookie_path: &str) -> anyhow::Result<Self> {
		let cookie = fs::read_to_string(cookie_path)?;
		let parts: Vec<&str> = cookie.trim().split(':').collect();
		if parts.len() != 2 {
			anyhow::bail!("Invalid cookie format");
		}

		Self::new(rpc_url, parts[0], parts[1])
	}

	pub fn new_default() -> anyhow::Result<Self> {
		let rpc_url =
			env::var("BITCOIND_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:18443".to_string());

		// Try cookie auth first - check multiple locations
		let cookie_path = env::var("BITCOIND_COOKIE").ok().or_else(|| {
			let home = env::var("HOME").ok()?;

			// macOS default: ~/Library/Application Support/Bitcoin/regtest/.cookie
			let macos_path =
				PathBuf::from(&home).join("Library/Application Support/Bitcoin/regtest/.cookie");
			if macos_path.exists() {
				return Some(macos_path.to_string_lossy().to_string());
			}

			// Linux default: ~/.bitcoin/regtest/.cookie
			let linux_path = PathBuf::from(&home).join(".bitcoin/regtest/.cookie");
			if linux_path.exists() {
				return Some(linux_path.to_string_lossy().to_string());
			}

			None
		});

		if let Some(cookie_path) = cookie_path {
			return Self::new_with_cookie(&rpc_url, &cookie_path);
		}

		// Fall back to user/pass from env
		let rpc_user = env::var("BITCOIND_RPC_USER").unwrap_or_else(|_| "user".to_string());
		let rpc_password =
			env::var("BITCOIND_RPC_PASSWORD").unwrap_or_else(|_| "password".to_string());

		Self::new(&rpc_url, &rpc_user, &rpc_password)
	}

	pub fn get_blockchain_info(&self) -> anyhow::Result<BlockchainInfo> {
		let info = self
			.client
			.get_blockchain_info()
			.map_err(|e| anyhow::anyhow!("get_blockchain_info failed: {}", e))?;
		Ok(BlockchainInfo { blocks: info.blocks as u64 })
	}

	/// Mine blocks to a specific address (use for funding nodes on regtest)
	pub fn mine_to_address(&self, num_blocks: u32, address: &str) -> anyhow::Result<()> {
		let addr = Address::from_str(address)
			.map_err(|e| anyhow::anyhow!("Invalid address: {}", e))?
			.assume_checked();

		self.client
			.generate_to_address(num_blocks as usize, &addr)
			.map_err(|e| anyhow::anyhow!("generate_to_address failed: {}", e))?;

		Ok(())
	}

	/// Mine blocks to a dummy address (just for block production)
	pub fn mine_blocks(&self, num_blocks: u32) -> anyhow::Result<()> {
		// Use a burn address for regtest
		let addr = Address::from_str("bcrt1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqdku202")
			.unwrap()
			.assume_checked();

		self.client
			.generate_to_address(num_blocks as usize, &addr)
			.map_err(|e| anyhow::anyhow!("generate_to_address failed: {}", e))?;

		Ok(())
	}
}
