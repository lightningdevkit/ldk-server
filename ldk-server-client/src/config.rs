// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Shared `ldk-server` client configuration.
//!
//! Parses the TOML configuration file used by the `ldk-server` daemon and exposes helpers for
//! locating the server's TLS certificate and macaroon on disk, so multiple clients (CLI, MCP
//! bridge, etc.) can resolve connection credentials in a consistent way.

use std::io::{self, ErrorKind, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const DEFAULT_CONFIG_FILE: &str = "config.toml";
const DEFAULT_CERT_FILE: &str = "tls.crt";
const CONFIG_FILE_SIZE_LIMIT: usize = 1024 * 1024;
const TLS_CERT_FILE_SIZE_LIMIT: usize = 1024 * 1024;
const MACAROONS_DIR: &str = "macaroons";
const ADMIN_MACAROON_FILE: &str = "admin.macaroon";
const MACAROON_FILE_SIZE_LIMIT: usize = crate::macaroon::MAX_MACAROON_BYTES * 2;

/// Default address of the `ldk-server` gRPC endpoint when no explicit value is configured.
pub const DEFAULT_GRPC_SERVICE_ADDRESS: &str = "127.0.0.1:3536";

/// Returns the OS-specific default data directory used by `ldk-server`.
pub fn get_default_data_dir() -> Option<PathBuf> {
	#[cfg(target_os = "macos")]
	{
		#[allow(deprecated)] // todo can remove once we update MSRV to 1.87+
		std::env::home_dir().map(|home| home.join("Library/Application Support/ldk-server"))
	}
	#[cfg(target_os = "windows")]
	{
		std::env::var("APPDATA").ok().map(|appdata| PathBuf::from(appdata).join("ldk-server"))
	}
	#[cfg(not(any(target_os = "macos", target_os = "windows")))]
	{
		#[allow(deprecated)] // todo can remove once we update MSRV to 1.87+
		std::env::home_dir().map(|home| home.join(".ldk-server"))
	}
}

/// Default path of the `ldk-server` configuration TOML file inside the default data directory.
pub fn get_default_config_path() -> Option<PathBuf> {
	get_default_data_dir().map(|dir| dir.join(DEFAULT_CONFIG_FILE))
}

/// Default path of the server's TLS certificate inside the default data directory.
pub fn get_default_cert_path() -> Option<PathBuf> {
	get_default_data_dir().map(|path| path.join(DEFAULT_CERT_FILE))
}

/// Default admin macaroon path for this network.
pub fn get_default_admin_macaroon_path(network: &str) -> Option<PathBuf> {
	get_default_data_dir()
		.map(|path| path.join(network).join(MACAROONS_DIR).join(ADMIN_MACAROON_FILE))
}

/// Admin macaroon path for this storage directory and network.
pub fn admin_macaroon_path_for_storage_dir(storage_dir: &str, network: &str) -> PathBuf {
	PathBuf::from(storage_dir).join(network).join(MACAROONS_DIR).join(ADMIN_MACAROON_FILE)
}

/// Path of the server's TLS certificate inside the given storage directory.
pub fn cert_path_for_storage_dir(storage_dir: &str) -> PathBuf {
	PathBuf::from(storage_dir).join(DEFAULT_CERT_FILE)
}

/// Top-level structure of the `ldk-server` configuration TOML file.
#[derive(Debug, Deserialize)]
pub struct Config {
	/// Node-level configuration.
	pub node: NodeConfig,
	/// Optional TLS configuration.
	pub tls: Option<TlsConfig>,
	/// Optional storage configuration.
	pub storage: Option<StorageConfig>,
}

/// `[tls]` section of the configuration file.
#[derive(Debug, Deserialize, Serialize)]
pub struct TlsConfig {
	/// Path to the server's TLS certificate in PEM format.
	pub cert_path: Option<String>,
}

/// `[node]` section of the configuration file.
#[derive(Debug, Deserialize)]
pub struct NodeConfig {
	/// Address of the `ldk-server` gRPC service.
	#[serde(default = "default_grpc_service_address")]
	pub grpc_service_address: String,
	network: String,
}

/// `[storage]` section of the configuration file.
#[derive(Debug, Deserialize)]
pub struct StorageConfig {
	/// On-disk storage configuration.
	pub disk: Option<DiskConfig>,
}

/// `[storage.disk]` section of the configuration file.
#[derive(Debug, Deserialize)]
pub struct DiskConfig {
	/// Directory used by the server to store its persistent data.
	pub dir_path: Option<String>,
}

impl Config {
	/// Returns the normalized Bitcoin network name configured for the node.
	pub fn network(&self) -> Result<String, String> {
		match self.node.network.as_str() {
			"bitcoin" | "mainnet" => Ok("bitcoin".to_string()),
			"testnet" => Ok("testnet".to_string()),
			"testnet4" => Ok("testnet4".to_string()),
			"signet" => Ok("signet".to_string()),
			"regtest" => Ok("regtest".to_string()),
			other => Err(format!("Unsupported network: {other}")),
		}
	}
}

/// Reads and parses the `ldk-server` configuration file at `path`.
pub fn load_config(path: &Path) -> Result<Config, String> {
	let contents = read_to_string_with_limit(path, CONFIG_FILE_SIZE_LIMIT)
		.map_err(|e| format!("Failed to read config file '{}': {}", path.display(), e))?;
	toml::from_str(&contents)
		.map_err(|e| format!("Failed to parse config file '{}': {}", path.display(), e))
}

/// Reads the server TLS certificate at `path`.
///
/// Returns an error if the file exceeds 1 MiB.
pub fn read_tls_certificate(path: &Path) -> Result<Vec<u8>, String> {
	read_with_limit(path, TLS_CERT_FILE_SIZE_LIMIT)
		.map_err(|e| format!("Failed to read server certificate file '{}': {e}", path.display()))
}

/// Resolves the base URL of the `ldk-server` gRPC endpoint.
///
/// Prefers `override_url`, falls back to the configuration file, and finally to
/// [`DEFAULT_GRPC_SERVICE_ADDRESS`].
pub fn resolve_base_url(override_url: Option<String>, config: Option<&Config>) -> String {
	override_url
		.or_else(|| config.map(|config| config.node.grpc_service_address.clone()))
		.unwrap_or_else(default_grpc_service_address)
}

/// Find the macaroon to use for requests.
///
/// Use `override_macaroon` if supplied. Otherwise, read `admin.macaroon` from the configured
/// storage directory, or the default data directory if no storage directory is configured.
/// Never fall back to a different instance when the selected file is missing.
/// Return an error for an unsupported network or an unreadable, oversized, or invalid token.
pub fn resolve_macaroon(
	override_macaroon: Option<String>, config: Option<&Config>,
) -> Result<Option<String>, String> {
	if override_macaroon.is_some() {
		return Ok(override_macaroon);
	}

	match resolve_macaroon_path(config)? {
		Some(path) => read_admin_macaroon(&path),
		None => Ok(None),
	}
}

/// Resolves the macaroon file path selected by [`resolve_macaroon`] when no override is given.
///
/// Uses the configured storage directory when one is set. Uses the OS-specific default data
/// directory only when no storage directory is configured.
///
/// Returns an error if the configured network is unsupported.
pub fn resolve_macaroon_path(config: Option<&Config>) -> Result<Option<PathBuf>, String> {
	let network = match config {
		Some(config) => config.network()?,
		None => "bitcoin".to_string(),
	};
	Ok(match storage_dir(config) {
		Some(dir) => Some(admin_macaroon_path_for_storage_dir(dir, &network)),
		None => get_default_admin_macaroon_path(&network),
	})
}

fn read_with_limit(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
	let file = std::fs::File::open(path)?;
	let mut contents = Vec::new();
	file.take(limit.saturating_add(1) as u64).read_to_end(&mut contents)?;
	if contents.len() > limit {
		return Err(io::Error::new(
			io::ErrorKind::InvalidData,
			format!("File '{}' exceeds the {limit} byte limit", path.display()),
		));
	}
	Ok(contents)
}

fn read_to_string_with_limit(path: &Path, limit: usize) -> io::Result<String> {
	String::from_utf8(read_with_limit(path, limit)?)
		.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn read_admin_macaroon(path: &Path) -> Result<Option<String>, String> {
	let contents = match read_to_string_with_limit(path, MACAROON_FILE_SIZE_LIMIT) {
		Ok(contents) => contents,
		Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
		Err(error) => {
			return Err(format!("Failed to read macaroon file '{}': {error}", path.display()))
		},
	};
	let token = contents.trim();
	crate::macaroon::parse_reusable_macaroon(token)
		.map_err(|error| format!("Invalid macaroon in '{}': {error}", path.display()))?;
	Ok(Some(token.to_string()))
}

/// Resolves the path to the server's TLS certificate (PEM).
///
/// Prefers `override_path`, falls back to `tls.cert_path` in the configuration file, then to the
/// certificate inside the configured storage directory (if present), and finally to the
/// OS-specific default path.
pub fn resolve_cert_path(
	override_path: Option<PathBuf>, config: Option<&Config>,
) -> Option<PathBuf> {
	override_path
		.or_else(|| {
			config
				.and_then(|c| c.tls.as_ref().and_then(|t| t.cert_path.as_ref().map(PathBuf::from)))
		})
		.or_else(|| storage_dir(config).map(cert_path_for_storage_dir).filter(|p| p.exists()))
		.or_else(get_default_cert_path)
}

fn storage_dir(config: Option<&Config>) -> Option<&str> {
	config.and_then(|c| c.storage.as_ref()?.disk.as_ref()?.dir_path.as_deref())
}

fn default_grpc_service_address() -> String {
	DEFAULT_GRPC_SERVICE_ADDRESS.to_string()
}

#[cfg(test)]
mod tests {
	use std::fs;
	use std::sync::atomic::{AtomicU32, Ordering};
	use std::sync::Mutex;
	use std::time::{SystemTime, UNIX_EPOCH};

	use super::{
		get_default_admin_macaroon_path, load_config, read_tls_certificate, resolve_base_url,
		resolve_macaroon, resolve_macaroon_path, Config, ADMIN_MACAROON_FILE,
		CONFIG_FILE_SIZE_LIMIT, DEFAULT_GRPC_SERVICE_ADDRESS, TLS_CERT_FILE_SIZE_LIMIT,
	};
	static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

	static ENV_LOCK: Mutex<()> = Mutex::new(());

	#[cfg(target_os = "windows")]
	fn set_default_data_dir(temp_dir: &std::path::Path) -> (String, Option<String>) {
		let old_value = std::env::var("APPDATA").ok();
		std::env::set_var("APPDATA", temp_dir);
		("APPDATA".to_string(), old_value)
	}

	#[cfg(not(target_os = "windows"))]
	fn set_default_data_dir(temp_dir: &std::path::Path) -> (String, Option<String>) {
		let old_value = std::env::var("HOME").ok();
		std::env::set_var("HOME", temp_dir);
		("HOME".to_string(), old_value)
	}

	fn restore_env_var(name: &str, value: Option<String>) {
		match value {
			Some(value) => std::env::set_var(name, value),
			None => std::env::remove_var(name),
		}
	}

	#[test]
	fn config_defaults_grpc_service_address() {
		let config: Config = toml::from_str(
			r#"
				[node]
				network = "regtest"
			"#,
		)
		.unwrap();

		assert_eq!(config.node.grpc_service_address, DEFAULT_GRPC_SERVICE_ADDRESS);
	}

	#[test]
	fn config_allows_server_config_fields() {
		let config = toml::from_str::<Config>(
			r#"
				[node]
				network = "regtest"
				listening_addresses = ["localhost:3001"]
				announcement_addresses = ["54.3.7.81:3001"]
				grpc_service_address = "127.0.0.1:3002"
				alias = "LDK Server"
				rgs_server_url = "https://rapidsync.lightningdevkit.org/snapshot/v2/"
				async_payments_role = "client"

				[tls]
				cert_path = "/path/to/tls.crt"
				key_path = "/path/to/tls.key"
				hosts = ["example.com", "ldk-server.local"]

				[storage.disk]
				dir_path = "/tmp"

				[log]
				level = "Trace"
				file = "/var/log/ldk-server.log"

				[bitcoind]
				rpc_address = "127.0.0.1:8332"
				rpc_user = "bitcoind-testuser"
				rpc_password = "bitcoind-testpassword"

				[[liquidity.lsps_client]]
				node_pubkey = "0217890e3aad8d35bc054f43acc00084b25229ecff0ab68debd82883ad65ee8266"
				address = "127.0.0.1:39735"
				token = "lsps2-token"
				trust_peer_0conf = true

				[liquidity.lsps2_service]
				advertise_service = false
				channel_opening_fee_ppm = 1000
				channel_over_provisioning_ppm = 500000
				min_channel_opening_fee_msat = 10000000
				min_channel_lifetime = 4320
				max_client_to_self_delay = 1440
				min_payment_size_msat = 10000000
				max_payment_size_msat = 25000000000
				client_trusts_lsp = true
				disable_client_reserve = false

				[tor]
				proxy_address = "127.0.0.1:9050"
			"#,
		)
		.unwrap();

		assert_eq!(config.network().unwrap(), "regtest");
		assert_eq!(config.node.grpc_service_address, "127.0.0.1:3002");
		assert_eq!(config.tls.unwrap().cert_path.unwrap(), "/path/to/tls.crt");
		assert_eq!(config.storage.unwrap().disk.unwrap().dir_path.unwrap(), "/tmp");
	}

	#[test]
	fn resolve_base_url_uses_cli_arg_first() {
		let config: Config = toml::from_str(
			r#"
				[node]
				network = "regtest"
				grpc_service_address = "127.0.0.1:3002"
			"#,
		)
		.unwrap();

		assert_eq!(
			resolve_base_url(Some("127.0.0.1:4000".to_string()), Some(&config)),
			"127.0.0.1:4000"
		);
	}

	#[test]
	fn resolve_base_url_falls_back_to_default() {
		assert_eq!(resolve_base_url(None, None), DEFAULT_GRPC_SERVICE_ADDRESS);
	}

	#[test]
	fn resolve_macaroon_rejects_unsupported_network() {
		let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
		let storage_dir = std::env::temp_dir()
			.join(format!("ldk-server-client-invalid-network-{}-{nonce}", std::process::id()));
		fs::create_dir_all(storage_dir.join("bitcoin").join("macaroons")).unwrap();
		fs::write(
			storage_dir.join("bitcoin").join("macaroons").join(ADMIN_MACAROON_FILE),
			crate::macaroon::Macaroon::mint(b"test root", b"test id").unwrap().to_hex(),
		)
		.unwrap();

		let config: Config = toml::from_str(&format!(
			r#"
				[node]
				network = "invalid-network"

				[storage.disk]
				dir_path = "{}"
			"#,
			storage_dir.display()
		))
		.unwrap();

		assert_eq!(
			resolve_macaroon_path(Some(&config)).unwrap_err(),
			"Unsupported network: invalid-network"
		);
		assert_eq!(
			resolve_macaroon(None, Some(&config)).unwrap_err(),
			"Unsupported network: invalid-network"
		);

		fs::remove_dir_all(storage_dir).unwrap();
	}

	#[test]
	fn resolve_macaroon_does_not_cross_storage_instances() {
		let _lock = ENV_LOCK.lock().unwrap();
		let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
		let temp_dir = std::env::temp_dir()
			.join(format!("ldk-server-client-key-instance-{}-{nonce}", std::process::id()));
		let configured_storage = temp_dir.join("configured");
		fs::create_dir_all(&configured_storage).unwrap();

		let (default_dir_env_var, old_default_dir) = set_default_data_dir(&temp_dir);
		let default_macaroon = get_default_admin_macaroon_path("regtest").unwrap();
		fs::create_dir_all(default_macaroon.parent().unwrap()).unwrap();
		fs::write(
			&default_macaroon,
			crate::macaroon::Macaroon::mint(b"test root", b"test id").unwrap().to_hex(),
		)
		.unwrap();

		let config: Config = toml::from_str(&format!(
			r#"
				[node]
				network = "regtest"

				[storage.disk]
				dir_path = "{}"
			"#,
			configured_storage.display()
		))
		.unwrap();

		assert_eq!(
			resolve_macaroon_path(Some(&config)).unwrap(),
			Some(configured_storage.join("regtest").join("macaroons").join(ADMIN_MACAROON_FILE))
		);
		let resolved = resolve_macaroon(None, Some(&config));

		restore_env_var(&default_dir_env_var, old_default_dir);
		fs::remove_dir_all(temp_dir).unwrap();
		assert!(resolved.unwrap().is_none());
	}

	#[test]
	fn read_tls_certificate_rejects_oversized_file() {
		let path = std::env::temp_dir()
			.join(format!("ldk-server-client-oversized-cert-{}", std::process::id()));
		std::fs::write(&path, vec![0; TLS_CERT_FILE_SIZE_LIMIT + 1]).unwrap();

		let error = read_tls_certificate(&path).unwrap_err();
		assert!(error.contains("exceeds"));

		std::fs::remove_file(path).unwrap();
	}

	#[test]
	fn load_config_rejects_oversized_file() {
		let path = std::env::temp_dir()
			.join(format!("ldk-server-client-oversized-config-{}", std::process::id()));
		std::fs::write(&path, vec![b'a'; CONFIG_FILE_SIZE_LIMIT + 1]).unwrap();

		let error = load_config(&path).unwrap_err();
		assert!(error.contains("exceeds"));

		std::fs::remove_file(path).unwrap();
	}

	#[test]
	fn resolve_macaroon_reads_scoped_admin_file() {
		let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
		let directory = std::env::temp_dir()
			.join(format!("ldk-server-client-config-test-{}-{count}", std::process::id()));
		let admin_directory = directory.join("regtest").join("macaroons");
		fs::create_dir_all(&admin_directory).unwrap();
		let token = "0201000207746573742d6964000006203846eea2ed59d53650493222c2380a3540668ade20044e28f9cb1e0b5b4e4429".to_string();
		fs::write(admin_directory.join("admin.macaroon"), &token).unwrap();
		let config: Config = toml::from_str(&format!(
			r#"
				[node]
				network = "regtest"

				[storage.disk]
				dir_path = "{}"
			"#,
			directory.display()
		))
		.unwrap();

		assert_eq!(resolve_macaroon(None, Some(&config)).unwrap(), Some(token.clone()));
		let admin_path = admin_directory.join("admin.macaroon");
		let bound =
			crate::macaroon::bind_macaroon_to_request(&token, "GetNodeInfo", &[0; 5]).unwrap();
		fs::write(&admin_path, &bound).unwrap();
		let error = resolve_macaroon(None, Some(&config)).unwrap_err();
		assert!(error.contains("request-bound macaroon cannot be used as a reusable credential"));
		assert!(!error.contains(&bound));
		for contents in [
			"not hexadecimal".to_string(),
			"deadbeef".to_string(),
			"00".repeat(super::MACAROON_FILE_SIZE_LIMIT),
		] {
			fs::write(&admin_path, contents).unwrap();
			assert!(resolve_macaroon(None, Some(&config)).is_err());
		}
		fs::remove_file(&admin_path).unwrap();
		assert_eq!(super::read_admin_macaroon(&admin_path).unwrap(), None);

		fs::remove_dir_all(directory).unwrap();
	}
}
