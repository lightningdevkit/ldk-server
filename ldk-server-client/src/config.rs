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
//! locating the server's TLS certificate and API key on disk, so multiple clients (CLI, MCP
//! bridge, etc.) can resolve connection credentials in a consistent way.

use std::path::PathBuf;

use hex_conservative::DisplayHex;
use serde::{Deserialize, Serialize};

const DEFAULT_CONFIG_FILE: &str = "config.toml";
const DEFAULT_CERT_FILE: &str = "tls.crt";
const API_KEY_FILE: &str = "api_key";

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

/// Default path of the network-scoped API key file inside the default data directory.
pub fn get_default_api_key_path(network: &str) -> Option<PathBuf> {
	get_default_data_dir().map(|path| path.join(network).join(API_KEY_FILE))
}

/// Path of the network-scoped API key file inside the given storage directory.
pub fn api_key_path_for_storage_dir(storage_dir: &str, network: &str) -> PathBuf {
	PathBuf::from(storage_dir).join(network).join(API_KEY_FILE)
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
pub fn load_config(path: &PathBuf) -> Result<Config, String> {
	let contents = std::fs::read_to_string(path)
		.map_err(|e| format!("Failed to read config file '{}': {}", path.display(), e))?;
	toml::from_str(&contents)
		.map_err(|e| format!("Failed to parse config file '{}': {}", path.display(), e))
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

/// Resolves the API key used to authenticate against the `ldk-server` gRPC endpoint.
///
/// Prefers `override_key`, falls back to reading the API key file from the configured storage
/// directory, and finally from the OS-specific default data directory. The raw bytes read from
/// disk are lower-hex encoded before being returned.
pub fn resolve_api_key(override_key: Option<String>, config: Option<&Config>) -> Option<String> {
	override_key.or_else(|| {
		let network =
			config.and_then(|c| c.network().ok()).unwrap_or_else(|| "bitcoin".to_string());
		storage_dir(config)
			.map(|dir| api_key_path_for_storage_dir(dir, &network))
			.and_then(|path| std::fs::read(&path).ok())
			.or_else(|| {
				get_default_api_key_path(&network).and_then(|path| std::fs::read(&path).ok())
			})
			.map(|bytes| bytes.to_lower_hex_string())
	})
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
	use super::{resolve_base_url, Config, DEFAULT_GRPC_SERVICE_ADDRESS};

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
}
