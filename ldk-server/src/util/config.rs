// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::{fs, io};

use clap::Parser;
use ldk_node::bitcoin::Network;
use ldk_node::lightning::ln::msgs::SocketAddress;
use ldk_node::lightning::routing::gossip::NodeAlias;
use ldk_node::liquidity::LSPS2ServiceConfig;
use log::LevelFilter;
use serde::{Deserialize, Serialize};

use crate::get_default_data_dir;

const DEFAULT_CONFIG_FILE: &str = "config.toml";

fn get_default_config_path() -> Option<PathBuf> {
	get_default_data_dir().map(|data_dir| data_dir.join(DEFAULT_CONFIG_FILE))
}

/// Configuration for LDK Server.
#[derive(Debug)]
pub struct Config {
	pub listening_addrs: Option<Vec<SocketAddress>>,
	pub announcement_addrs: Option<Vec<SocketAddress>>,
	pub alias: Option<NodeAlias>,
	pub network: Network,
	pub tls_config: Option<TlsConfig>,
	pub rest_service_addr: SocketAddr,
	pub storage_dir_path: Option<String>,
	pub chain_source: ChainSource,
	pub rabbitmq_connection_string: String,
	pub rabbitmq_exchange_name: String,
	pub lsps2_service_config: Option<LSPS2ServiceConfig>,
	pub log_level: LevelFilter,
	pub log_file_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsConfig {
	pub cert_path: Option<String>,
	pub key_path: Option<String>,
	pub hosts: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ChainSource {
	Rpc { rpc_address: SocketAddr, rpc_user: String, rpc_password: String },
	Electrum { server_url: String },
	Esplora { server_url: String },
}

/// A builder for `Config`.
#[derive(Default)]
struct ConfigBuilder {
	listening_addresses: Option<Vec<String>>,
	announcement_addresses: Option<Vec<String>>,
	alias: Option<String>,
	network: Option<Network>,
	tls_config: Option<TlsConfig>,
	rest_service_address: Option<String>,
	storage_dir_path: Option<String>,
	electrum_url: Option<String>,
	esplora_url: Option<String>,
	bitcoind_rpc_addr: Option<String>,
	bitcoind_rpc_user: Option<String>,
	bitcoind_rpc_password: Option<String>,
	rabbitmq_connection_string: Option<String>,
	rabbitmq_exchange_name: Option<String>,
	lsps2: Option<LiquidityConfig>,
	log_level: Option<String>,
	log_file_path: Option<String>,
}

impl ConfigBuilder {
	fn merge_toml(&mut self, toml: TomlConfig) {
		if let Some(node) = toml.node {
			self.network = node.network.or(self.network);
			self.listening_addresses =
				node.listening_addresses.or(self.listening_addresses.clone());
			self.announcement_addresses =
				node.announcement_addresses.or(self.announcement_addresses.clone());
			self.rest_service_address =
				node.rest_service_address.or(self.rest_service_address.clone());
			self.alias = node.alias.or(self.alias.clone());
		}

		if let Some(storage) = toml.storage {
			self.storage_dir_path =
				storage.disk.and_then(|d| d.dir_path).or(self.storage_dir_path.clone());
		}

		if let Some(bitcoind) = toml.bitcoind {
			self.bitcoind_rpc_addr = bitcoind.rpc_address.or(self.bitcoind_rpc_addr.clone());
			self.bitcoind_rpc_user = bitcoind.rpc_user.or(self.bitcoind_rpc_user.clone());
			self.bitcoind_rpc_password =
				bitcoind.rpc_password.or(self.bitcoind_rpc_password.clone());
		}

		if let Some(electrum) = toml.electrum {
			self.electrum_url = Some(electrum.server_url);
		}

		if let Some(esplora) = toml.esplora {
			self.esplora_url = Some(esplora.server_url);
		}

		if let Some(log) = toml.log {
			self.log_level = log.level.or(self.log_level.clone());
			self.log_file_path = log.file.or(self.log_file_path.clone());
		}

		if let Some(rabbitmq) = toml.rabbitmq {
			self.rabbitmq_connection_string = Some(rabbitmq.connection_string);
			self.rabbitmq_exchange_name = Some(rabbitmq.exchange_name);
		}

		if let Some(liquidity) = toml.liquidity {
			self.lsps2 = Some(liquidity);
		}

		if let Some(tls) = toml.tls {
			self.tls_config = Some(TlsConfig {
				cert_path: tls.cert_path,
				key_path: tls.key_path,
				hosts: tls.hosts.unwrap_or_default(),
			});
		}
	}

	fn merge_args(&mut self, args: &ArgsConfig) {
		if let Some(network) = args.node_network {
			self.network = Some(network);
		}

		if let Some(node_listening_addresses) = &args.node_listening_addresses {
			self.listening_addresses = Some(node_listening_addresses.clone());
		}

		if let Some(node_announcement_addresses) = &args.node_announcement_addresses {
			self.announcement_addresses = Some(node_announcement_addresses.clone());
		}

		if let Some(node_rest_service_address) = &args.node_rest_service_address {
			self.rest_service_address = Some(node_rest_service_address.clone());
		}

		if let Some(node_alias) = &args.node_alias {
			self.alias = Some(node_alias.clone());
		}

		if let Some(bitcoind_rpc_address) = &args.bitcoind_rpc_address {
			self.bitcoind_rpc_addr = Some(bitcoind_rpc_address.clone());
		}

		if let Some(bitcoind_rpc_user) = &args.bitcoind_rpc_user {
			self.bitcoind_rpc_user = Some(bitcoind_rpc_user.clone());
		}

		if let Some(bitcoind_rpc_password) = &args.bitcoind_rpc_password {
			self.bitcoind_rpc_password = Some(bitcoind_rpc_password.clone());
		}

		if let Some(storage_dir_path) = &args.storage_dir_path {
			self.storage_dir_path = Some(storage_dir_path.clone());
		}
	}

	fn build(self) -> io::Result<Config> {
		let network = self.network.ok_or_else(|| missing_field_err("network"))?;

		let rest_service_addr = self
			.rest_service_address
			.ok_or_else(|| missing_field_err("rest_service_address"))?
			.parse::<SocketAddr>()
			.map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

		let listening_addrs: Option<Vec<SocketAddress>> = self
			.listening_addresses
			.map(|addrs| {
				addrs
					.into_iter()
					.map(|addr| {
						SocketAddress::from_str(&addr).map_err(|e| {
							io::Error::new(
								io::ErrorKind::InvalidInput,
								format!("Invalid listening addresses configured: {}", e),
							)
						})
					})
					.collect::<Result<Vec<_>, _>>()
			})
			.transpose()?;

		let announcement_addrs: Option<Vec<SocketAddress>> = self
			.announcement_addresses
			.map(|addrs| {
				addrs
					.into_iter()
					.map(|addr| {
						SocketAddress::from_str(&addr).map_err(|e| {
							io::Error::new(
								io::ErrorKind::InvalidInput,
								format!("Invalid announcement addresses configured: {}", e),
							)
						})
					})
					.collect::<Result<Vec<_>, _>>()
			})
			.transpose()?;

		let alias = self
			.alias
			.map(|alias_str| {
				let node_alias = parse_alias(alias_str.as_ref()).map_err(|e| {
					io::Error::new(e.kind(), format!("Failed to parse alias: {}", e))
				})?;
				Ok::<NodeAlias, io::Error>(node_alias)
			})
			.transpose()?;

		let rpc_configured = self.bitcoind_rpc_addr.is_some()
			|| self.bitcoind_rpc_user.is_some()
			|| self.bitcoind_rpc_password.is_some();
		let electrum_configured = self.electrum_url.is_some();
		let esplora_configured = self.esplora_url.is_some();

		let configured_sources_count = [rpc_configured, electrum_configured, esplora_configured]
			.iter()
			.filter(|&&is_configured| is_configured)
			.count();

		if configured_sources_count != 1 {
			return Err(io::Error::new(
				io::ErrorKind::InvalidInput,
				"Must set a single chain source, multiple were configured".to_string(),
			));
		}

		let chain_source = if rpc_configured {
			let rpc_address = self
				.bitcoind_rpc_addr
				.ok_or_else(|| missing_field_err("bitcoind_rpc_address"))?
				.parse::<SocketAddr>()
				.map_err(|e| {
					io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid RPC addr: {}", e))
				})?;

			let rpc_user =
				self.bitcoind_rpc_user.ok_or_else(|| missing_field_err("bitcoind_rpc_user"))?;

			let rpc_password = self
				.bitcoind_rpc_password
				.ok_or_else(|| missing_field_err("bitcoind_rpc_password"))?;

			ChainSource::Rpc { rpc_address, rpc_user, rpc_password }
		} else if let Some(url) = self.electrum_url {
			ChainSource::Electrum { server_url: url }
		} else if let Some(url) = self.esplora_url {
			ChainSource::Esplora { server_url: url }
		} else {
			return Err(io::Error::new(io::ErrorKind::InvalidInput, "No valid Chain Source configured. Provide Bitcoind RPC, Electrum, or Esplora details."));
		};

		let log_level = self
			.log_level
			.as_ref()
			.map(|level_str| {
				LevelFilter::from_str(level_str).map_err(|e| {
					io::Error::new(
						io::ErrorKind::InvalidInput,
						format!("Invalid log level configured: {}", e),
					)
				})
			})
			.transpose()?
			.unwrap_or(LevelFilter::Debug);

		#[cfg(feature = "events-rabbitmq")]
		let (rabbitmq_connection_string, rabbitmq_exchange_name) = {
			let connection_string = self.rabbitmq_connection_string.ok_or_else(|| io::Error::new(
				io::ErrorKind::InvalidInput,
				"Both `rabbitmq.connection_string` and `rabbitmq.exchange_name` must be configured if enabling `events-rabbitmq` feature."
			))?;
			let exchange_name = self.rabbitmq_exchange_name.ok_or_else(|| io::Error::new(
				io::ErrorKind::InvalidInput,
				"Both `rabbitmq.connection_string` and `rabbitmq.exchange_name` must be configured if enabling `events-rabbitmq` feature."
			))?;

			if connection_string.is_empty() || exchange_name.is_empty() {
				return Err(io::Error::new(
					io::ErrorKind::InvalidInput,
					"Both `rabbitmq.connection_string` and `rabbitmq.exchange_name` must be configured if enabling `events-rabbitmq` feature."
				));
			}

			(connection_string, exchange_name)
		};

		#[cfg(not(feature = "events-rabbitmq"))]
		let (rabbitmq_connection_string, rabbitmq_exchange_name) = (String::new(), String::new());

		#[cfg(feature = "experimental-lsps2-support")]
		let lsps2_service_config = {
			let liquidity = self.lsps2.ok_or_else(|| io::Error::new(
				io::ErrorKind::InvalidInput,
				"`liquidity.lsps2_service` must be defined in config if enabling `experimental-lsps2-support` feature."
			))?;
			let lsps2_service = liquidity.lsps2_service.ok_or_else(|| io::Error::new(
				io::ErrorKind::InvalidInput,
				"`liquidity.lsps2_service` must be defined in config if enabling `experimental-lsps2-support` feature."
			))?;
			Some(lsps2_service.into())
		};

		#[cfg(not(feature = "experimental-lsps2-support"))]
		let lsps2_service_config = None;

		Ok(Config {
			network,
			listening_addrs,
			announcement_addrs,
			alias,
			tls_config: self.tls_config,
			rest_service_addr,
			storage_dir_path: self.storage_dir_path,
			chain_source,
			rabbitmq_connection_string,
			rabbitmq_exchange_name,
			lsps2_service_config,
			log_level,
			log_file_path: self.log_file_path,
		})
	}
}

/// Configuration loaded from a TOML file.
#[derive(Deserialize, Serialize)]
pub struct TomlConfig {
	node: Option<NodeConfig>,
	storage: Option<StorageConfig>,
	bitcoind: Option<BitcoindConfig>,
	electrum: Option<ElectrumConfig>,
	esplora: Option<EsploraConfig>,
	rabbitmq: Option<RabbitmqConfig>,
	liquidity: Option<LiquidityConfig>,
	log: Option<LogConfig>,
	tls: Option<TomlTlsConfig>,
}

#[derive(Deserialize, Serialize)]
struct NodeConfig {
	network: Option<Network>,
	listening_addresses: Option<Vec<String>>,
	announcement_addresses: Option<Vec<String>>,
	rest_service_address: Option<String>,
	alias: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct StorageConfig {
	disk: Option<DiskConfig>,
}

#[derive(Deserialize, Serialize)]
struct DiskConfig {
	dir_path: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct BitcoindConfig {
	rpc_address: Option<String>,
	rpc_user: Option<String>,
	rpc_password: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct ElectrumConfig {
	server_url: String,
}

#[derive(Deserialize, Serialize)]
struct EsploraConfig {
	server_url: String,
}

#[derive(Deserialize, Serialize)]
struct LogConfig {
	level: Option<String>,
	file: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct RabbitmqConfig {
	connection_string: String,
	exchange_name: String,
}

#[derive(Deserialize, Serialize)]
struct TomlTlsConfig {
	cert_path: Option<String>,
	key_path: Option<String>,
	hosts: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize)]
struct LiquidityConfig {
	lsps2_service: Option<LSPS2ServiceTomlConfig>,
}

#[derive(Deserialize, Serialize, Debug)]
struct LSPS2ServiceTomlConfig {
	advertise_service: bool,
	channel_opening_fee_ppm: u32,
	channel_over_provisioning_ppm: u32,
	min_channel_opening_fee_msat: u64,
	min_channel_lifetime: u32,
	max_client_to_self_delay: u32,
	min_payment_size_msat: u64,
	max_payment_size_msat: u64,
	client_trusts_lsp: bool,
	require_token: Option<String>,
}

impl From<LSPS2ServiceTomlConfig> for LSPS2ServiceConfig {
	fn from(val: LSPS2ServiceTomlConfig) -> Self {
		let LSPS2ServiceTomlConfig {
			advertise_service,
			channel_opening_fee_ppm,
			channel_over_provisioning_ppm,
			min_channel_opening_fee_msat,
			min_channel_lifetime,
			max_client_to_self_delay,
			min_payment_size_msat,
			max_payment_size_msat,
			client_trusts_lsp,
			require_token,
		} = val;

		Self {
			advertise_service,
			channel_opening_fee_ppm,
			channel_over_provisioning_ppm,
			min_channel_opening_fee_msat,
			min_channel_lifetime,
			min_payment_size_msat,
			max_client_to_self_delay,
			max_payment_size_msat,
			client_trusts_lsp,
			require_token,
		}
	}
}

#[derive(Parser, Debug)]
#[command(
	version,
	about = "LDK Server Configuration",
	long_about = None,
	override_usage = "ldk-server [config_path]"
)]
pub struct ArgsConfig {
	#[arg(required = false, help = "The configuration file for running LDK Server.")]
	config_file: Option<String>,

	#[arg(
		long,
		env = "LDK_SERVER_NODE_NETWORK",
		help = "The used Bitcoin network for the underlying Bitcoin node."
	)]
	node_network: Option<Network>,

	#[arg(
		long,
		env = "LDK_SERVER_NODE_LISTENING_ADDRESSES",
		help = "The addresses on which the node will listen for incoming connections."
	)]
	node_listening_addresses: Option<Vec<String>>,

	#[arg(
		long,
		env = "LDK_SERVER_NODE_ANNOUNCEMENT_ADDRESSES",
		help = "The addresses which the node will announce to the gossip network that it accepts connections on."
	)]
	node_announcement_addresses: Option<Vec<String>>,

	#[arg(
		long,
		env = "LDK_SERVER_NODE_REST_SERVICE_ADDRESS",
		help = "The rest service address for the LDK Server API."
	)]
	node_rest_service_address: Option<String>,

	#[arg(
		long,
		env = "LDK_SERVER_NODE_ALIAS",
		help = "The node alias that will be used when broadcasting announcements to the gossip network."
	)]
	node_alias: Option<String>,

	#[arg(
		long,
		env = "LDK_SERVER_BITCOIND_RPC_ADDRESS",
		help = "The underlying Bitcoin node RPC address."
	)]
	bitcoind_rpc_address: Option<String>,

	#[arg(
		long,
		env = "LDK_SERVER_BITCOIND_RPC_USER",
		help = "The underlying Bitcoin node RPC user."
	)]
	bitcoind_rpc_user: Option<String>,

	#[arg(
		long,
		env = "LDK_SERVER_BITCOIND_RPC_PASSWORD",
		help = "The underlying Bitcoin node RPC password."
	)]
	bitcoind_rpc_password: Option<String>,

	#[arg(
		long,
		env = "LDK_SERVER_STORAGE_DIR_PATH",
		help = "The path where the underlying LDK and BDK persist their data."
	)]
	storage_dir_path: Option<String>,
}

pub fn load_config(args: &ArgsConfig) -> io::Result<Config> {
	let mut builder = ConfigBuilder::default();

	let config_file = if let Some(path) = &args.config_file {
		Some(PathBuf::from(path))
	} else {
		get_default_config_path().filter(|path| path.exists())
	};

	if let Some(path) = config_file {
		let content = fs::read_to_string(&path).map_err(|e| {
			io::Error::new(e.kind(), format!("Failed to read config file '{:?}': {}", path, e))
		})?;
		let toml_config: TomlConfig = toml::from_str(&content).map_err(|e| {
			io::Error::new(
				io::ErrorKind::InvalidData,
				format!("Config file contains invalid TOML format: {}", e),
			)
		})?;

		builder.merge_toml(toml_config);
	}

	builder.merge_args(args);

	builder.build()
}

fn missing_field_err(field: &str) -> io::Error {
	io::Error::new(
		io::ErrorKind::InvalidInput,
		format!(
			"Missing `{}`. Please provide it via config file, CLI argument, or environment variable.",
			field
		),
	)
}

fn parse_alias(alias_str: &str) -> Result<NodeAlias, io::Error> {
	let mut bytes = [0u8; 32];
	let alias_bytes = alias_str.trim().as_bytes();
	if alias_bytes.len() > 32 {
		return Err(io::Error::new(
			io::ErrorKind::InvalidInput,
			"node.alias must be at most 32 bytes long.".to_string(),
		));
	}
	bytes[..alias_bytes.len()].copy_from_slice(alias_bytes);
	Ok(NodeAlias(bytes))
}

#[cfg(test)]
mod tests {
	use std::str::FromStr;

	use ldk_node::bitcoin::Network;
	use ldk_node::lightning::ln::msgs::SocketAddress;

	use super::*;
	use crate::util::config::{load_config, ArgsConfig};
	const DEFAULT_CONFIG: &str = r#"
				[node]
				network = "regtest"
				listening_addresses = ["localhost:3001"]
				announcement_addresses = ["54.3.7.81:3001"]
				rest_service_address = "127.0.0.1:3002"
				alias = "LDK Server"

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

				[rabbitmq]
				connection_string = "rabbitmq_connection_string"
				exchange_name = "rabbitmq_exchange_name"

				[liquidity.lsps2_service]
				advertise_service = false
				channel_opening_fee_ppm = 1000            # 0.1% fee
				channel_over_provisioning_ppm = 500000    # 50% extra capacity
				min_channel_opening_fee_msat = 10000000   # 10,000 satoshis
				min_channel_lifetime = 4320               # ~30 days
				max_client_to_self_delay = 1440           # ~10 days
				min_payment_size_msat = 10000000          # 10,000 satoshis
				max_payment_size_msat = 25000000000       # 0.25 BTC
				client_trusts_lsp = true
				"#;

	fn default_args_config() -> ArgsConfig {
		ArgsConfig {
			config_file: None,
			node_network: Some(Network::Regtest),
			node_listening_addresses: Some(vec!["localhost:3008".to_string()]),
			node_announcement_addresses: Some(vec!["54.3.7.81:3001".to_string()]),
			node_rest_service_address: Some(String::from("127.0.0.1:3009")),
			bitcoind_rpc_address: Some(String::from("127.0.1.9:18443")),
			bitcoind_rpc_user: Some(String::from("bitcoind-testuser_cli")),
			bitcoind_rpc_password: Some(String::from("bitcoind-testpassword_cli")),
			storage_dir_path: Some(String::from("/tmp_cli")),
			node_alias: Some(String::from("LDK Server CLI")),
		}
	}

	fn empty_args_config() -> ArgsConfig {
		ArgsConfig {
			config_file: None,
			node_network: None,
			node_listening_addresses: None,
			node_announcement_addresses: None,
			node_rest_service_address: None,
			node_alias: None,
			bitcoind_rpc_address: None,
			bitcoind_rpc_user: None,
			bitcoind_rpc_password: None,
			storage_dir_path: None,
		}
	}

	fn missing_field_msg(field: &str) -> String {
		format!(
			"Missing `{}`. Please provide it via config file, CLI argument, or environment variable.",
			field
		)
	}

	#[test]
	fn test_config_from_file() {
		let storage_path = std::env::temp_dir();
		let config_file_name = "test_config_from_file.toml";

		fs::write(storage_path.join(config_file_name), DEFAULT_CONFIG).unwrap();

		let mut args_config = empty_args_config();
		args_config.config_file =
			Some(storage_path.join(config_file_name).to_string_lossy().to_string());

		let config = load_config(&args_config).unwrap();

		let alias = "LDK Server";

		#[cfg(feature = "events-rabbitmq")]
		let (expected_rabbit_conn, expected_rabbit_exchange) =
			("rabbitmq_connection_string".to_string(), "rabbitmq_exchange_name".to_string());

		#[cfg(not(feature = "events-rabbitmq"))]
		let (expected_rabbit_conn, expected_rabbit_exchange) = (String::new(), String::new());

		let expected = Config {
			listening_addrs: Some(vec![SocketAddress::from_str("localhost:3001").unwrap()]),
			announcement_addrs: Some(vec![SocketAddress::from_str("54.3.7.81:3001").unwrap()]),
			alias: Some(parse_alias(alias).unwrap()),
			network: Network::Regtest,
			rest_service_addr: SocketAddr::from_str("127.0.0.1:3002").unwrap(),
			storage_dir_path: Some("/tmp".to_string()),
			tls_config: Some(TlsConfig {
				cert_path: Some("/path/to/tls.crt".to_string()),
				key_path: Some("/path/to/tls.key".to_string()),
				hosts: vec!["example.com".to_string(), "ldk-server.local".to_string()],
			}),
			chain_source: ChainSource::Rpc {
				rpc_address: SocketAddr::from_str("127.0.0.1:8332").unwrap(),
				rpc_user: "bitcoind-testuser".to_string(),
				rpc_password: "bitcoind-testpassword".to_string(),
			},
			rabbitmq_connection_string: expected_rabbit_conn,
			rabbitmq_exchange_name: expected_rabbit_exchange,
			lsps2_service_config: Some(LSPS2ServiceConfig {
				require_token: None,
				advertise_service: false,
				channel_opening_fee_ppm: 1000,
				channel_over_provisioning_ppm: 500000,
				min_channel_opening_fee_msat: 10000000,
				min_channel_lifetime: 4320,
				max_client_to_self_delay: 1440,
				min_payment_size_msat: 10000000,
				max_payment_size_msat: 25000000000,
				client_trusts_lsp: true,
			}),
			log_level: LevelFilter::Trace,
			log_file_path: Some("/var/log/ldk-server.log".to_string()),
		};

		assert_eq!(config.listening_addrs, expected.listening_addrs);
		assert_eq!(config.announcement_addrs, expected.announcement_addrs);
		assert_eq!(config.alias, expected.alias);
		assert_eq!(config.network, expected.network);
		assert_eq!(config.rest_service_addr, expected.rest_service_addr);
		assert_eq!(config.storage_dir_path, expected.storage_dir_path);
		assert_eq!(config.chain_source, expected.chain_source);
		assert_eq!(config.rabbitmq_connection_string, expected.rabbitmq_connection_string);
		assert_eq!(config.rabbitmq_exchange_name, expected.rabbitmq_exchange_name);
		#[cfg(feature = "experimental-lsps2-support")]
		assert_eq!(config.lsps2_service_config.is_some(), expected.lsps2_service_config.is_some());
		assert_eq!(config.log_level, expected.log_level);
		assert_eq!(config.log_file_path, expected.log_file_path);

		// Test case where only electrum is set

		let toml_config = r#"
			[node]
			network = "regtest"
			listening_addresses = ["localhost:3001"]
			announcement_addresses = ["54.3.7.81:3001"]
			rest_service_address = "127.0.0.1:3002"
			alias = "LDK Server"

			[tls]
			cert_path = "/path/to/tls.crt"
			key_path = "/path/to/tls.key"
			hosts = ["example.com", "ldk-server.local"]

			[storage.disk]
			dir_path = "/tmp"

			[log]
			level = "Trace"
			file = "/var/log/ldk-server.log"

			[electrum]
			server_url = "ssl://electrum.blockstream.info:50002"

			[rabbitmq]
			connection_string = "rabbitmq_connection_string"
			exchange_name = "rabbitmq_exchange_name"

			[liquidity.lsps2_service]
			advertise_service = false
			channel_opening_fee_ppm = 1000            # 0.1% fee
			channel_over_provisioning_ppm = 500000    # 50% extra capacity
			min_channel_opening_fee_msat = 10000000   # 10,000 satoshis
			min_channel_lifetime = 4320               # ~30 days
			max_client_to_self_delay = 1440           # ~10 days
			min_payment_size_msat = 10000000          # 10,000 satoshis
			max_payment_size_msat = 25000000000       # 0.25 BTC
			client_trusts_lsp = true
			"#;

		fs::write(storage_path.join(config_file_name), toml_config).unwrap();
		let config = load_config(&args_config).unwrap();

		let ChainSource::Electrum { server_url } = config.chain_source else {
			panic!("unexpected chain source");
		};

		assert_eq!(server_url, "ssl://electrum.blockstream.info:50002");

		// Test case where only bitcoind is set

		let toml_config = r#"
			[node]
			network = "regtest"
			listening_addresses = ["localhost:3001"]
			announcement_addresses = ["54.3.7.81:3001"]
			rest_service_address = "127.0.0.1:3002"
			alias = "LDK Server"

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
			rpc_address = "127.0.0.1:8332"    # RPC endpoint
			rpc_user = "bitcoind-testuser"
			rpc_password = "bitcoind-testpassword"

			[rabbitmq]
			connection_string = "rabbitmq_connection_string"
			exchange_name = "rabbitmq_exchange_name"

			[liquidity.lsps2_service]
			advertise_service = false
			channel_opening_fee_ppm = 1000            # 0.1% fee
			channel_over_provisioning_ppm = 500000    # 50% extra capacity
			min_channel_opening_fee_msat = 10000000   # 10,000 satoshis
			min_channel_lifetime = 4320               # ~30 days
			max_client_to_self_delay = 1440           # ~10 days
			min_payment_size_msat = 10000000          # 10,000 satoshis
			max_payment_size_msat = 25000000000       # 0.25 BTC
			client_trusts_lsp = true
			"#;

		fs::write(storage_path.join(config_file_name), toml_config).unwrap();
		let config = load_config(&args_config).unwrap();

		let ChainSource::Rpc { rpc_address, rpc_user, rpc_password } = config.chain_source else {
			panic!("unexpected chain source");
		};

		assert_eq!(rpc_address, SocketAddr::from_str("127.0.0.1:8332").unwrap());
		assert_eq!(rpc_user, "bitcoind-testuser");
		assert_eq!(rpc_password, "bitcoind-testpassword");

		// Test case where both bitcoind and esplora are set, resulting in an error

		let toml_config = r#"
			[node]
			network = "regtest"
			listening_addresses = ["localhost:3001"]
			announcement_addresses = ["54.3.7.81:3001"]
			rest_service_address = "127.0.0.1:3002"
			alias = "LDK Server"

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
			rpc_address = "127.0.0.1:8332"    # RPC endpoint
			rpc_user = "bitcoind-testuser"
			rpc_password = "bitcoind-testpassword"

			[esplora]
			server_url = "https://mempool.space/api"

			[rabbitmq]
			connection_string = "rabbitmq_connection_string"
			exchange_name = "rabbitmq_exchange_name"

			[liquidity.lsps2_service]
			advertise_service = false
			channel_opening_fee_ppm = 1000            # 0.1% fee
			channel_over_provisioning_ppm = 500000    # 50% extra capacity
			min_channel_opening_fee_msat = 10000000   # 10,000 satoshis
			min_channel_lifetime = 4320               # ~30 days
			max_client_to_self_delay = 1440           # ~10 days
			min_payment_size_msat = 10000000          # 10,000 satoshis
			max_payment_size_msat = 25000000000       # 0.25 BTC
			client_trusts_lsp = true
			"#;

		fs::write(storage_path.join(config_file_name), toml_config).unwrap();
		let error = load_config(&args_config).unwrap_err();
		assert_eq!(error.to_string(), "Must set a single chain source, multiple were configured");
	}

	#[test]
	fn test_config_optional_values() {
		let storage_path = std::env::temp_dir();
		let config_file_name = "test_only_required_config.toml";

		let mut args_config = empty_args_config();
		args_config.config_file =
			Some(storage_path.join(config_file_name).to_string_lossy().to_string());

		// Test with optional values not specified in the config file
		let toml_config = r#"
			[node]
			network = "regtest"
			rest_service_address = "127.0.0.1:3002"

			[bitcoind]
			rpc_address = "127.0.0.1:8332"    # RPC endpoint
			rpc_user = "bitcoind-testuser"
			rpc_password = "bitcoind-testpassword"

			[rabbitmq]
			connection_string = "rabbitmq_connection_string"
			exchange_name = "rabbitmq_exchange_name"

			[liquidity.lsps2_service]
			advertise_service = false
			channel_opening_fee_ppm = 1000            # 0.1% fee
			channel_over_provisioning_ppm = 500000    # 50% extra capacity
			min_channel_opening_fee_msat = 10000000   # 10,000 satoshis
			min_channel_lifetime = 4320               # ~30 days
			max_client_to_self_delay = 1440           # ~10 days
			min_payment_size_msat = 10000000          # 10,000 satoshis
			max_payment_size_msat = 25000000000       # 0.25 BTC
			client_trusts_lsp = true
			"#;

		fs::write(storage_path.join(config_file_name), toml_config).unwrap();
		assert!(load_config(&args_config).is_ok());
	}

	#[test]
	fn test_config_missing_fields_in_file() {
		let storage_path = std::env::temp_dir();
		let config_file_name = "test_config_missing_fields_in_file.toml";

		let mut args_config = empty_args_config();
		args_config.config_file =
			Some(storage_path.join(config_file_name).to_string_lossy().to_string());

		macro_rules! validate_missing {
			($field:expr, $err_msg:expr) => {
				let mut toml_config = DEFAULT_CONFIG.to_string();
				toml_config = remove_config_line(&toml_config, $field);
				fs::write(storage_path.join(config_file_name), &toml_config).unwrap();
				let result = load_config(&args_config);
				assert!(result.is_err());
				let err = result.unwrap_err();
				assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
				assert_eq!(err.to_string(), $err_msg);
			};
		}

		#[cfg(feature = "experimental-lsps2-support")]
		{
			validate_missing!(
				"[liquidity.lsps2_service]",
				"`liquidity.lsps2_service` must be defined in config if enabling `experimental-lsps2-support` feature."
			);
		}

		#[cfg(feature = "events-rabbitmq")]
		{
			validate_missing!(
				"[rabbitmq]",
				"Both `rabbitmq.connection_string` and `rabbitmq.exchange_name` must be configured if enabling `events-rabbitmq` feature."
			);
		}

		validate_missing!("rpc_password", missing_field_msg("bitcoind_rpc_password"));
		validate_missing!("rpc_user", missing_field_msg("bitcoind_rpc_user"));
		validate_missing!("rpc_address", missing_field_msg("bitcoind_rpc_address"));
		validate_missing!("rest_service_address =", missing_field_msg("rest_service_address"));
		validate_missing!("network =", missing_field_msg("network"));
	}

	fn remove_config_line(config: &str, key: &str) -> String {
		config
			.lines()
			.filter(|line| !line.trim_start().starts_with(key))
			.collect::<Vec<_>>()
			.join("\n")
	}

	#[test]
	#[cfg(not(feature = "experimental-lsps2-support"))]
	#[cfg(not(feature = "events-rabbitmq"))]
	fn test_config_from_args_config() {
		let args_config = default_args_config();
		let config = load_config(&args_config).unwrap();

		let expected = Config {
			listening_addrs: Some(vec![SocketAddress::from_str(
				&args_config.node_listening_addresses.as_ref().unwrap()[0],
			)
			.unwrap()]),
			announcement_addrs: Some(vec![SocketAddress::from_str(
				&args_config.node_announcement_addresses.as_ref().unwrap()[0],
			)
			.unwrap()]),
			network: Network::Regtest,
			rest_service_addr: SocketAddr::from_str(
				args_config.node_rest_service_address.as_deref().unwrap(),
			)
			.unwrap(),
			alias: Some(parse_alias(args_config.node_alias.as_deref().unwrap()).unwrap()),
			storage_dir_path: Some(args_config.storage_dir_path.unwrap()),
			tls_config: None,
			chain_source: ChainSource::Rpc {
				rpc_address: SocketAddr::from_str(
					args_config.bitcoind_rpc_address.as_deref().unwrap(),
				)
				.unwrap(),
				rpc_user: args_config.bitcoind_rpc_user.unwrap(),
				rpc_password: args_config.bitcoind_rpc_password.unwrap(),
			},
			rabbitmq_connection_string: String::new(),
			rabbitmq_exchange_name: String::new(),
			lsps2_service_config: None,
			log_level: LevelFilter::Trace,
			log_file_path: Some("/var/log/ldk-server.log".to_string()),
		};

		assert_eq!(config.listening_addrs, expected.listening_addrs);
		assert_eq!(config.announcement_addrs, expected.announcement_addrs);
		assert_eq!(config.network, expected.network);
		assert_eq!(config.rest_service_addr, expected.rest_service_addr);
		assert_eq!(config.storage_dir_path, expected.storage_dir_path);
		assert_eq!(config.chain_source, expected.chain_source);
		assert_eq!(config.rabbitmq_connection_string, expected.rabbitmq_connection_string);
		assert_eq!(config.rabbitmq_exchange_name, expected.rabbitmq_exchange_name);
		assert!(config.lsps2_service_config.is_none());
	}

	#[test]
	#[cfg(not(feature = "experimental-lsps2-support"))]
	#[cfg(not(feature = "events-rabbitmq"))]
	fn test_config_missing_fields_in_args_config() {
		macro_rules! validate_missing {
			($field:ident, $err_msg:expr) => {
				let mut args_config = default_args_config();
				args_config.$field = None;
				let result = load_config(&args_config);
				assert!(result.is_err());
				let err = result.unwrap_err();
				assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
				assert_eq!(err.to_string(), $err_msg);
			};
		}

		validate_missing!(bitcoind_rpc_password, missing_field_msg("bitcoind_rpc_password"));
		validate_missing!(bitcoind_rpc_user, missing_field_msg("bitcoind_rpc_user"));
		validate_missing!(bitcoind_rpc_address, missing_field_msg("bitcoind_rpc_address"));
		validate_missing!(node_network, missing_field_msg("network"));
		validate_missing!(node_rest_service_address, missing_field_msg("rest_service_address"));
	}

	#[test]
	fn test_args_config_overrides_file() {
		let storage_path = std::env::temp_dir();
		let config_file_name = "test_args_config_overrides_file.toml";

		fs::write(storage_path.join(config_file_name), DEFAULT_CONFIG).unwrap();
		let mut args_config: ArgsConfig = default_args_config();
		args_config.config_file =
			Some(storage_path.join(config_file_name).to_string_lossy().to_string());

		#[cfg(feature = "events-rabbitmq")]
		let (expected_rabbit_conn, expected_rabbit_exchange) =
			("rabbitmq_connection_string".to_string(), "rabbitmq_exchange_name".to_string());

		#[cfg(not(feature = "events-rabbitmq"))]
		let (expected_rabbit_conn, expected_rabbit_exchange) = (String::new(), String::new());

		let config = load_config(&args_config).unwrap();
		let expected = Config {
			listening_addrs: Some(vec![SocketAddress::from_str(
				&args_config.node_listening_addresses.as_ref().unwrap()[0],
			)
			.unwrap()]),
			announcement_addrs: Some(vec![SocketAddress::from_str(
				&args_config.node_announcement_addresses.as_ref().unwrap()[0],
			)
			.unwrap()]),
			network: Network::Regtest,
			rest_service_addr: SocketAddr::from_str(
				args_config.node_rest_service_address.as_deref().unwrap(),
			)
			.unwrap(),
			alias: Some(parse_alias(args_config.node_alias.as_deref().unwrap()).unwrap()),
			storage_dir_path: Some(args_config.storage_dir_path.unwrap()),
			tls_config: Some(TlsConfig {
				cert_path: Some("/path/to/tls.crt".to_string()),
				key_path: Some("/path/to/tls.key".to_string()),
				hosts: vec!["example.com".to_string(), "ldk-server.local".to_string()],
			}),
			chain_source: ChainSource::Rpc {
				rpc_address: SocketAddr::from_str(
					args_config.bitcoind_rpc_address.as_deref().unwrap(),
				)
				.unwrap(),
				rpc_user: args_config.bitcoind_rpc_user.unwrap(),
				rpc_password: args_config.bitcoind_rpc_password.unwrap(),
			},
			rabbitmq_connection_string: expected_rabbit_conn,
			rabbitmq_exchange_name: expected_rabbit_exchange,
			lsps2_service_config: Some(LSPS2ServiceConfig {
				require_token: None,
				advertise_service: false,
				channel_opening_fee_ppm: 1000,
				channel_over_provisioning_ppm: 500000,
				min_channel_opening_fee_msat: 10000000,
				min_channel_lifetime: 4320,
				max_client_to_self_delay: 1440,
				min_payment_size_msat: 10000000,
				max_payment_size_msat: 25000000000,
				client_trusts_lsp: true,
			}),
			log_level: LevelFilter::Trace,
			log_file_path: Some("/var/log/ldk-server.log".to_string()),
		};

		assert_eq!(config.listening_addrs, expected.listening_addrs);
		assert_eq!(config.announcement_addrs, expected.announcement_addrs);
		assert_eq!(config.network, expected.network);
		assert_eq!(config.rest_service_addr, expected.rest_service_addr);
		assert_eq!(config.storage_dir_path, expected.storage_dir_path);
		assert_eq!(config.chain_source, expected.chain_source);
		assert_eq!(config.rabbitmq_connection_string, expected.rabbitmq_connection_string);
		assert_eq!(config.rabbitmq_exchange_name, expected.rabbitmq_exchange_name);
		#[cfg(feature = "experimental-lsps2-support")]
		assert_eq!(config.lsps2_service_config.is_some(), expected.lsps2_service_config.is_some());
	}

	#[test]
	#[cfg(feature = "events-rabbitmq")]
	fn test_error_if_rabbitmq_feature_without_valid_config_file() {
		let args_config = empty_args_config();
		let result = load_config(&args_config);
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
	}

	#[test]
	#[cfg(feature = "experimental-lsps2-support")]
	fn test_error_if_lsps2_feature_without_valid_config_file() {
		let args_config = empty_args_config();
		let result = load_config(&args_config);
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
	}
}
