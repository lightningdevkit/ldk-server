// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

mod api;
mod grpc_service;
mod io;
mod util;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use hex::DisplayHex;
use ldk_node::bitcoin::Network;
use ldk_node::config::Config;
use ldk_node::entropy::NodeEntropy;
use ldk_node::lightning::ln::channelmanager::PaymentId;
use ldk_node::{Builder, Event, Node};
use ldk_server_protos::api::lightning_node_server::LightningNodeServer;
use ldk_server_protos::events;
use ldk_server_protos::events::{event_envelope, EventEnvelope};
use ldk_server_protos::types::Payment;
use log::{debug, error, info};
use prost::Message;
use tokio::select;
use tokio::signal::unix::SignalKind;
use tokio::sync::broadcast;
use tonic::transport::{Identity, ServerTlsConfig};

use crate::grpc_service::{AuthInterceptor, NodeGrpcService};
use crate::io::persist::paginated_kv_store::PaginatedKVStore;
use crate::io::persist::sqlite_store::SqliteStore;
use crate::io::persist::{
	FORWARDED_PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
	FORWARDED_PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE, PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
	PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
};
use crate::util::config::{load_config, ArgsConfig, ChainSource};
use crate::util::logger::ServerLogger;
use crate::util::proto_adapter::{forwarded_payment_to_proto, payment_to_proto};
use crate::util::systemd;
use crate::util::tls::get_or_generate_tls_pem;

const API_KEY_FILE: &str = "api_key";

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

fn main() {
	let args_config = ArgsConfig::parse();

	let mut ldk_node_config = Config::default();
	let config_file = match load_config(&args_config) {
		Ok(config) => config,
		Err(e) => {
			eprintln!("Invalid configuration: {e}");
			std::process::exit(-1);
		},
	};

	let storage_dir: PathBuf = match config_file.storage_dir_path {
		None => {
			let default = get_default_data_dir();
			match default {
				Some(path) => {
					info!("No storage_dir_path configured, defaulting to {}", path.display());
					path
				},
				None => {
					eprintln!("Unable to determine home directory for default storage path.");
					std::process::exit(-1);
				},
			}
		},
		Some(configured_path) => PathBuf::from(configured_path),
	};

	let network_dir: PathBuf = match config_file.network {
		Network::Bitcoin => storage_dir.join("bitcoin"),
		Network::Testnet => storage_dir.join("testnet"),
		Network::Testnet4 => storage_dir.join("testnet4"),
		Network::Signet => storage_dir.join("signet"),
		Network::Regtest => storage_dir.join("regtest"),
	};

	let log_file_path = config_file.log_file_path.map(PathBuf::from).unwrap_or_else(|| {
		let mut default_log_path = network_dir.clone();
		default_log_path.push("ldk-server.log");
		default_log_path
	});

	if log_file_path == storage_dir || log_file_path == network_dir {
		eprintln!("Log file path cannot be the same as storage directory path.");
		std::process::exit(-1);
	}

	let logger = match ServerLogger::init(config_file.log_level, &log_file_path) {
		Ok(logger) => logger,
		Err(e) => {
			eprintln!("Failed to initialize logger: {e}");
			std::process::exit(-1);
		},
	};

	let api_key = match load_or_generate_api_key(&network_dir) {
		Ok(key) => key,
		Err(e) => {
			eprintln!("Failed to load or generate API key: {e}");
			std::process::exit(-1);
		},
	};

	ldk_node_config.storage_dir_path = network_dir.to_str().unwrap().to_string();
	ldk_node_config.listening_addresses = config_file.listening_addrs;
	ldk_node_config.announcement_addresses = config_file.announcement_addrs;
	ldk_node_config.network = config_file.network;

	let mut builder = Builder::from_config(ldk_node_config);
	builder.set_log_facade_logger();

	if let Some(alias) = config_file.alias {
		if let Err(e) = builder.set_node_alias(alias.to_string()) {
			error!("Failed to set node alias: {e}");
			std::process::exit(-1);
		}
	}

	match config_file.chain_source {
		ChainSource::Rpc { rpc_host, rpc_port, rpc_user, rpc_password } => {
			builder.set_chain_source_bitcoind_rpc(rpc_host, rpc_port, rpc_user, rpc_password);
		},
		ChainSource::Electrum { server_url } => {
			builder.set_chain_source_electrum(server_url, None);
		},
		ChainSource::Esplora { server_url } => {
			builder.set_chain_source_esplora(server_url, None);
		},
	}

	if let Some(pathfinding_scores_source) = config_file.pathfinding_scores_source_url {
		builder.set_pathfinding_scores_source(pathfinding_scores_source);
	}

	if let Some(rgs_server_url) = config_file.rgs_server_url {
		builder.set_gossip_source_rgs(rgs_server_url);
	}

	if let Some(lsps2_client_config) = config_file.lsps2_client_config {
		builder.set_liquidity_source_lsps2(
			lsps2_client_config.node_id,
			lsps2_client_config.address,
			lsps2_client_config.token,
		);
	}

	// LSPS2 support is highly experimental and for testing purposes only.
	#[cfg(feature = "experimental-lsps2-support")]
	builder.set_liquidity_provider_lsps2(
		config_file.lsps2_service_config.expect("Missing liquidity.lsps2_server config"),
	);

	let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
		Ok(runtime) => Arc::new(runtime),
		Err(e) => {
			error!("Failed to setup tokio runtime: {e}");
			std::process::exit(-1);
		},
	};

	builder.set_runtime(runtime.handle().clone());

	let seed_path = storage_dir.join("keys_seed").to_str().unwrap().to_string();
	let node_entropy = match NodeEntropy::from_seed_path(seed_path) {
		Ok(entropy) => entropy,
		Err(e) => {
			error!("Failed to load or generate seed: {e}");
			std::process::exit(-1);
		},
	};

	let node = match builder.build(node_entropy) {
		Ok(node) => Arc::new(node),
		Err(e) => {
			error!("Failed to build LDK Node: {e}");
			std::process::exit(-1);
		},
	};

	let paginated_store: Arc<dyn PaginatedKVStore> =
		Arc::new(match SqliteStore::new(network_dir.clone(), None, None) {
			Ok(store) => store,
			Err(e) => {
				error!("Failed to create SqliteStore: {e:?}");
				std::process::exit(-1);
			},
		});

	let (event_sender, _) = broadcast::channel::<EventEnvelope>(1024);

	info!("Starting up...");
	match node.start() {
		Ok(()) => {},
		Err(e) => {
			error!("Failed to start up LDK Node: {e}");
			std::process::exit(-1);
		},
	}

	let addrs = node
		.config()
		.announcement_addresses
		.filter(|a| !a.is_empty())
		.or(node.config().listening_addresses);
	if let Some(addresses) = addrs {
		for address in &addresses {
			info!("NODE_URI: {}@{}", node.node_id(), address);
		}
	}

	runtime.block_on(async {
		let mut sighup_stream = match tokio::signal::unix::signal(SignalKind::hangup()) {
			Ok(stream) => stream,
			Err(e) => {
				error!("Failed to register SIGHUP handler: {e}");
				std::process::exit(-1);
			}
		};

		let mut sigterm_stream = match tokio::signal::unix::signal(SignalKind::terminate()) {
			Ok(stream) => stream,
			Err(e) => {
				error!("Failed to register for SIGTERM stream: {e}");
				std::process::exit(-1);
			}
		};

		let tls_pem = match get_or_generate_tls_pem(
			config_file.tls_config,
			storage_dir.to_str().unwrap(),
		) {
			Ok(pem) => pem,
			Err(e) => {
				error!("Failed to set up TLS: {e}");
				std::process::exit(-1);
			}
		};

		let identity = Identity::from_pem(&tls_pem.cert_pem, &tls_pem.key_pem);
		let tls_config = ServerTlsConfig::new().identity(identity);

		let grpc_service = NodeGrpcService::new(
			Arc::clone(&node),
			Arc::clone(&paginated_store),
			event_sender.clone(),
		);

		let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

		let incoming = tonic::transport::server::TcpIncoming::new(
			config_file.grpc_service_addr, true, None,
		).unwrap_or_else(|e| {
			error!("Failed to bind gRPC service to {}: {e}", config_file.grpc_service_addr);
			std::process::exit(-1);
		});

		let auth_interceptor = AuthInterceptor::new(api_key);
		let grpc_server = tonic::transport::Server::builder()
			.tls_config(tls_config)
			.expect("Failed to configure TLS for gRPC server")
			.add_service(LightningNodeServer::with_interceptor(grpc_service, auth_interceptor))
			.serve_with_incoming_shutdown(incoming, async {
				let _ = shutdown_rx.await;
			});

		info!("gRPC service listening on {} with TLS", config_file.grpc_service_addr);
		systemd::notify_ready();

		let event_node = Arc::clone(&node);

		let grpc_handle = tokio::spawn(grpc_server);

		// Event processing and signal handling loop
		loop {
			select! {
				event = event_node.next_event_async() => {
						match event {
							Event::ChannelPending { channel_id, counterparty_node_id, .. } => {
								info!(
									"CHANNEL_PENDING: {} from counterparty {}",
									channel_id, counterparty_node_id
								);
								if let Err(e) = event_node.event_handled() {
									error!("Failed to mark event as handled: {e}");
								}
							},
							Event::ChannelReady { channel_id, counterparty_node_id, .. } => {
								info!(
									"CHANNEL_READY: {} from counterparty {:?}",
									channel_id, counterparty_node_id
								);
								if let Err(e) = event_node.event_handled() {
									error!("Failed to mark event as handled: {e}");
								}
							},
							Event::PaymentReceived { payment_id, payment_hash, amount_msat, .. } => {
								info!(
									"PAYMENT_RECEIVED: with id {:?}, hash {}, amount_msat {}",
									payment_id, payment_hash, amount_msat
								);
								let payment_id = payment_id.expect("PaymentId expected for ldk-server >=0.1");

								publish_event_and_upsert_payment(&payment_id,
									|payment_ref| event_envelope::Event::PaymentReceived(events::PaymentReceived {
										payment: Some(payment_ref.clone()),
									}),
									&event_node,
									&event_sender,
									Arc::clone(&paginated_store));
							},
							Event::PaymentSuccessful {payment_id, ..} => {
								let payment_id = payment_id.expect("PaymentId expected for ldk-server >=0.1");

								publish_event_and_upsert_payment(&payment_id,
									|payment_ref| event_envelope::Event::PaymentSuccessful(events::PaymentSuccessful {
										payment: Some(payment_ref.clone()),
									}),
									&event_node,
									&event_sender,
									Arc::clone(&paginated_store));
							},
							Event::PaymentFailed {payment_id, ..} => {
								let payment_id = payment_id.expect("PaymentId expected for ldk-server >=0.1");

								publish_event_and_upsert_payment(&payment_id,
									|payment_ref| event_envelope::Event::PaymentFailed(events::PaymentFailed {
										payment: Some(payment_ref.clone()),
									}),
									&event_node,
									&event_sender,
									Arc::clone(&paginated_store));
							},
							Event::PaymentClaimable {payment_id, ..} => {
								publish_event_and_upsert_payment(&payment_id,
									|payment_ref| event_envelope::Event::PaymentClaimable(events::PaymentClaimable {
										payment: Some(payment_ref.clone()),
									}),
									&event_node,
									&event_sender,
									Arc::clone(&paginated_store));
							},
							Event::PaymentForwarded {
								prev_channel_id,
								next_channel_id,
								prev_user_channel_id,
								next_user_channel_id,
								prev_node_id,
								next_node_id,
								total_fee_earned_msat,
								skimmed_fee_msat,
								claim_from_onchain_tx,
								outbound_amount_forwarded_msat
							} => {

								info!("PAYMENT_FORWARDED: with outbound_amount_forwarded_msat {}, total_fee_earned_msat: {}, inbound channel: {}, outbound channel: {}",
									outbound_amount_forwarded_msat.unwrap_or(0), total_fee_earned_msat.unwrap_or(0), prev_channel_id, next_channel_id
								);

								let forwarded_payment = forwarded_payment_to_proto(
									prev_channel_id,
									next_channel_id,
									prev_user_channel_id,
									next_user_channel_id,
									prev_node_id,
									next_node_id,
									total_fee_earned_msat,
									skimmed_fee_msat,
									claim_from_onchain_tx,
									outbound_amount_forwarded_msat
								);

								let mut forwarded_payment_id = [0u8; 32];
								getrandom::getrandom(&mut forwarded_payment_id).expect("Failed to generate random bytes");

								let forwarded_payment_creation_time = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time must be > 1970").as_secs() as i64;

								if let Err(e) = event_sender.send(EventEnvelope {
									event: Some(event_envelope::Event::PaymentForwarded(events::PaymentForwarded {
										forwarded_payment: Some(forwarded_payment.clone()),
									})),
								}) {
									debug!("No event subscribers connected: {e}");
								}

								match paginated_store.write(FORWARDED_PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,FORWARDED_PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
									&forwarded_payment_id.to_lower_hex_string(),
									forwarded_payment_creation_time,
									&forwarded_payment.encode_to_vec(),
								) {
									Ok(_) => {
										if let Err(e) = event_node.event_handled() {
											error!("Failed to mark event as handled: {e}");
										}
									}
									Err(e) => {
											error!("Failed to write forwarded payment to persistence: {}", e);
									}
								}
							},
							_ => {
								if let Err(e) = event_node.event_handled() {
									error!("Failed to mark event as handled: {e}");
								}
							},
						}
					},
					_ = sighup_stream.recv() => {
						if let Err(e) = logger.reopen() {
							error!("Failed to reopen log file on SIGHUP: {e}");
						}
					}
					_ = tokio::signal::ctrl_c() => {
						info!("Received CTRL-C, shutting down..");
						break;
					}
					_ = sigterm_stream.recv() => {
						info!("Received SIGTERM, shutting down..");
						break;
					}
				}
			}

		let _ = shutdown_tx.send(());
		if let Err(e) = grpc_handle.await {
			error!("gRPC server error: {e}");
		}
	});

	systemd::notify_stopping();
	node.stop().expect("Shutdown should always succeed.");
	info!("Shutdown complete..");
}

fn publish_event_and_upsert_payment(
	payment_id: &PaymentId, payment_to_event: fn(&Payment) -> event_envelope::Event,
	event_node: &Node, event_sender: &broadcast::Sender<EventEnvelope>,
	paginated_store: Arc<dyn PaginatedKVStore>,
) {
	if let Some(payment_details) = event_node.payment(payment_id) {
		let payment = payment_to_proto(payment_details);

		let event = payment_to_event(&payment);
		if let Err(e) = event_sender.send(EventEnvelope { event: Some(event) }) {
			debug!("No event subscribers connected: {e}");
		}

		upsert_payment_details(event_node, Arc::clone(&paginated_store), &payment);
	} else {
		error!("Unable to find payment with paymentId: {payment_id}");
	}
}

fn upsert_payment_details(
	event_node: &Node, paginated_store: Arc<dyn PaginatedKVStore>, payment: &Payment,
) {
	let time =
		SystemTime::now().duration_since(UNIX_EPOCH).expect("Time must be > 1970").as_secs() as i64;

	match paginated_store.write(
		PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
		PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
		&payment.id,
		time,
		&payment.encode_to_vec(),
	) {
		Ok(_) => {
			if let Err(e) = event_node.event_handled() {
				error!("Failed to mark event as handled: {e}");
			}
		},
		Err(e) => {
			error!("Failed to write payment to persistence: {e}");
		},
	}
}

/// Loads the API key from a file, or generates a new one if it doesn't exist.
/// The API key file is stored with 0400 permissions (read-only for owner).
fn load_or_generate_api_key(storage_dir: &Path) -> std::io::Result<String> {
	let api_key_path = storage_dir.join(API_KEY_FILE);

	if api_key_path.exists() {
		let key_bytes = fs::read(&api_key_path)?;
		Ok(key_bytes.to_lower_hex_string())
	} else {
		// Ensure the storage directory exists
		fs::create_dir_all(storage_dir)?;

		// Generate a 32-byte random API key
		let mut key_bytes = [0u8; 32];
		getrandom::getrandom(&mut key_bytes).map_err(std::io::Error::other)?;

		// Write the raw bytes to the file
		fs::write(&api_key_path, key_bytes)?;

		// Set permissions to 0400 (read-only for owner)
		let permissions = fs::Permissions::from_mode(0o400);
		fs::set_permissions(&api_key_path, permissions)?;

		log::debug!("Generated new API key at {}", api_key_path.display());
		Ok(key_bytes.to_lower_hex_string())
	}
}
