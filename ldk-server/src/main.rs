// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

mod api;
mod io;
mod service;
mod util;

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use clap::Parser;
use hex::DisplayHex;
use hyper::server::conn::http2;
use hyper_util::rt::{TokioExecutor, TokioIo};
use ldk_node::bitcoin::Network;
use ldk_node::config::Config;
use ldk_node::entropy::NodeEntropy;
use ldk_node::lightning::events::ClosureReason;
use ldk_node::lightning::ln::channelmanager::PaymentId;
use ldk_node::{Builder, Event, Node};
use ldk_server_grpc::events;
use ldk_server_grpc::events::{event_envelope, EventEnvelope};
use ldk_server_grpc::types::Payment;
use log::{debug, error, info};
use prost::Message;
use tokio::net::TcpListener;
use tokio::select;
use tokio::signal::unix::SignalKind;
use tokio::sync::broadcast;

use crate::io::persist::paginated_kv_store::PaginatedKVStore;
use crate::io::persist::sqlite_store::SqliteStore;
use crate::io::persist::{
	FORWARDED_PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
	FORWARDED_PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE, PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
	PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
};
use crate::service::NodeService;
use crate::util::config::{load_config, ArgsConfig, ChainSource};
use crate::util::logger::ServerLogger;
use crate::util::metrics::Metrics;
use crate::util::proto_adapter::{forwarded_payment_to_proto, payment_to_proto};
use crate::util::systemd;
use crate::util::tls::get_or_generate_tls_config;

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

	if let Some(tor_config) = config_file.tor_config {
		let tor_config = ldk_node::config::TorConfig { proxy_address: tor_config.proxy_address };
		if let Err(e) = builder.set_tor_config(tor_config) {
			error!("Failed to configure Tor proxy: {e}");
			std::process::exit(-1);
		}
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
	let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

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
		// Register SIGHUP handler for log rotation
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
		let event_node = Arc::clone(&node);
		let mut ready_channel_ids: HashSet<String> = event_node
			.list_channels()
			.into_iter()
			.map(|channel| channel.channel_id.0.to_lower_hex_string())
			.collect();

		let metrics: Option<Arc<Metrics>> = if config_file.metrics_enabled {
			let poll_metrics_interval = Duration::from_secs(config_file.poll_metrics_interval.unwrap_or(60));
			let metrics_node = Arc::clone(&node);
			let mut interval = tokio::time::interval(poll_metrics_interval);
			let metrics = Arc::new(Metrics::new());
			let metrics_bg = Arc::clone(&metrics);

			// Initialize metrics that are event-driven to ensure they start with correct values from persistence
			metrics.initialize_payment_metrics(&metrics_node);

			runtime.spawn(async move {
				loop {
					interval.tick().await;
					metrics_bg.update_all_pollable_metrics(&metrics_node);
				}
			});
			Some(metrics)
		} else {
			None
		};

		let metrics_auth_header = if let (Some(username), Some(password)) =
			(config_file.metrics_username.as_ref(), config_file.metrics_password.as_ref())
		{
			let auth = format!("{}:{}", username, password);
			Some(format!("Basic {}", BASE64_STANDARD.encode(auth)))
		} else {
			None
		};

		let grpc_listener = TcpListener::bind(config_file.grpc_service_addr)
			.await
			.expect("Failed to bind listening port");

		let server_config = match get_or_generate_tls_config(
			config_file.tls_config,
			storage_dir.to_str().unwrap(),
		) {
			Ok(config) => config,
			Err(e) => {
				error!("Failed to set up TLS: {e}");
				std::process::exit(-1);
			}
		};
		let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));
		info!("gRPC service listening on {}", config_file.grpc_service_addr);

		systemd::notify_ready();

		loop {
			select! {
					event = event_node.next_event_async() => {
						match event {
							Event::ChannelPending {
								channel_id,
								user_channel_id,
								counterparty_node_id,
								funding_txo,
								..
							} => {
								info!(
									"CHANNEL_PENDING: {} from counterparty {}",
									channel_id, counterparty_node_id
								);

								send_channel_state_event(
									event_envelope::Event::ChannelStateChanged(events::ChannelStateChanged {
										channel_id: channel_id.0.to_lower_hex_string(),
										user_channel_id: user_channel_id.0.to_string(),
										counterparty_node_id: Some(counterparty_node_id.to_string()),
										state: events::ChannelState::Pending.into(),
										funding_txo: Some(funding_txo.to_string()),
										reason: None,
										closure_initiator: events::ChannelClosureInitiator::Unspecified.into(),
									}),
									&event_sender,
								);

								if let Err(e) = event_node.event_handled() {
									error!("Failed to mark event as handled: {e}");
								}
							},
							Event::ChannelReady {
								channel_id,
								user_channel_id,
								counterparty_node_id,
								funding_txo,
							} => {
								info!(
									"CHANNEL_READY: {} from counterparty {:?}",
									channel_id, counterparty_node_id
								);

								let channel_id_hex = channel_id.0.to_lower_hex_string();
								ready_channel_ids.insert(channel_id_hex.clone());

								send_channel_state_event(
									event_envelope::Event::ChannelStateChanged(events::ChannelStateChanged {
										channel_id: channel_id_hex,
										user_channel_id: user_channel_id.0.to_string(),
										counterparty_node_id: counterparty_node_id
											.map(|node_id| node_id.to_string()),
										state: events::ChannelState::Ready.into(),
										funding_txo: funding_txo.map(|outpoint| outpoint.to_string()),
										reason: None,
										closure_initiator: events::ChannelClosureInitiator::Unspecified.into(),
									}),
									&event_sender,
								);

								if let Err(e) = event_node.event_handled() {
									error!("Failed to mark event as handled: {e}");
								}

							if let Some(metrics) = &metrics {
								metrics.update_channels_count(false);
							}
						},
							Event::ChannelClosed {
								channel_id,
								user_channel_id,
								counterparty_node_id,
								reason,
							} => {
								info!(
									"CHANNEL_CLOSED: {} from counterparty {:?}",
									channel_id, counterparty_node_id
								);

								let channel_id_hex = channel_id.0.to_lower_hex_string();
								let was_ready = ready_channel_ids.remove(&channel_id_hex);
								let reason_ref = reason.as_ref();
								let is_open_failure = !was_ready && is_channel_open_failure(reason_ref);

								send_channel_state_event(
									event_envelope::Event::ChannelStateChanged(events::ChannelStateChanged {
										channel_id: channel_id_hex,
										user_channel_id: user_channel_id.0.to_string(),
										counterparty_node_id: counterparty_node_id
											.map(|node_id| node_id.to_string()),
										state: if is_open_failure {
											events::ChannelState::OpenFailed.into()
										} else {
											events::ChannelState::Closed.into()
										},
										funding_txo: None,
										reason: reason_ref.map(closure_reason_to_proto),
										closure_initiator: closure_initiator_from_reason(reason_ref).into(),
									}),
									&event_sender,
								);

								if let Err(e) = event_node.event_handled() {
									error!("Failed to mark event as handled: {e}");
								}

							if let Some(metrics) = &metrics {
								metrics.update_channels_count(true);
							}
						}
						Event::PaymentReceived { payment_id, payment_hash, amount_msat, .. } => {
							info!(
								"PAYMENT_RECEIVED: with id {:?}, hash {}, amount_msat {}",
								payment_id, payment_hash, amount_msat
							);
							let payment_id = payment_id.expect("PaymentId expected for ldk-server >=0.1");

							send_event_and_upsert_payment(&payment_id,
								|payment_ref| event_envelope::Event::PaymentReceived(events::PaymentReceived {
									payment: Some(payment_ref.clone()),
								}),
								&event_node,
								&event_sender,
								Arc::clone(&paginated_store));

							if let Some(metrics) = &metrics {
								metrics.update_all_balances(&event_node);
							}
						},
						Event::PaymentSuccessful {payment_id, ..} => {
							let payment_id = payment_id.expect("PaymentId expected for ldk-server >=0.1");

							send_event_and_upsert_payment(&payment_id,
								|payment_ref| event_envelope::Event::PaymentSuccessful(events::PaymentSuccessful {
									payment: Some(payment_ref.clone()),
								}),
								&event_node,
								&event_sender,
								Arc::clone(&paginated_store));

							if let Some(metrics) = &metrics {
								metrics.update_payments_count(true);
								metrics.update_all_balances(&event_node);
							}
						},
						Event::PaymentFailed {payment_id, ..} => {
							let payment_id = payment_id.expect("PaymentId expected for ldk-server >=0.1");

							send_event_and_upsert_payment(&payment_id,
								|payment_ref| event_envelope::Event::PaymentFailed(events::PaymentFailed {
									payment: Some(payment_ref.clone()),
								}),
								&event_node,
								&event_sender,
								Arc::clone(&paginated_store));

							if let Some(metrics) = &metrics {
								metrics.update_payments_count(false);
							}
						},
						Event::PaymentClaimable {payment_id, ..} => {
							send_event_and_upsert_payment(&payment_id,
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
								debug!("No event subscribers connected, skipping event: {e}");
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
				res = grpc_listener.accept() => {
					match res {
						Ok((stream, _)) => {
							let node_service = NodeService::new(
								Arc::clone(&node),
								Arc::clone(&paginated_store),
								api_key.clone(),
								metrics.clone(),
								metrics_auth_header.clone(),
								event_sender.clone(),
								shutdown_rx.clone(),
							);
							let acceptor = tls_acceptor.clone();
							runtime.spawn(async move {
								match acceptor.accept(stream).await {
									Ok(tls_stream) => {
										let io_stream = TokioIo::new(tls_stream);
										if let Err(err) = http2::Builder::new(TokioExecutor::new()).serve_connection(io_stream, node_service).await {
											error!("Failed to serve TLS connection: {err}");
										}
									},
									Err(e) => error!("TLS handshake failed: {e}"),
								}
							});
						},
						Err(e) => error!("Failed to accept connection: {}", e),
					}
				}
				_ = tokio::signal::ctrl_c() => {
					info!("Received CTRL-C, shutting down..");
					let _ = shutdown_tx.send(true);
					break;
				}
				_ = sighup_stream.recv() => {
					if let Err(e) = logger.reopen() {
						error!("Failed to reopen log file on SIGHUP: {e}");
					}
				}
				_ = sigterm_stream.recv() => {
					info!("Received SIGTERM, shutting down..");
					let _ = shutdown_tx.send(true);
					break;
				}
			}
		}
	});

	systemd::notify_stopping();
	node.stop().expect("Shutdown should always succeed.");
	info!("Shutdown complete..");
}

fn send_event_and_upsert_payment(
	payment_id: &PaymentId, payment_to_event: fn(&Payment) -> event_envelope::Event,
	event_node: &Node, event_sender: &broadcast::Sender<EventEnvelope>,
	paginated_store: Arc<dyn PaginatedKVStore>,
) {
	if let Some(payment_details) = event_node.payment(payment_id) {
		let payment = payment_to_proto(payment_details);

		let event = payment_to_event(&payment);
		if let Err(e) = event_sender.send(EventEnvelope { event: Some(event) }) {
			debug!("No event subscribers connected, skipping event: {e}");
		}

		upsert_payment_details(event_node, Arc::clone(&paginated_store), &payment);
	} else {
		error!("Unable to find payment with paymentId: {payment_id}");
	}
}

fn send_channel_state_event(
	event: event_envelope::Event, event_sender: &broadcast::Sender<EventEnvelope>,
) {
	if let Err(e) = event_sender.send(EventEnvelope { event: Some(event) }) {
		debug!("No event subscribers connected, skipping event: {e}");
	}
}

fn is_channel_open_failure(reason: Option<&ClosureReason>) -> bool {
	matches!(
		reason,
		Some(ClosureReason::FundingTimedOut)
			| Some(ClosureReason::DisconnectedPeer)
			| Some(ClosureReason::CounterpartyCoopClosedUnfundedChannel)
			| Some(ClosureReason::LocallyCoopClosedUnfundedChannel)
			| Some(ClosureReason::FundingBatchClosure)
	)
}

fn closure_initiator_from_reason(
	reason: Option<&ClosureReason>,
) -> events::ChannelClosureInitiator {
	match reason {
		Some(ClosureReason::HolderForceClosed { .. })
		| Some(ClosureReason::LocallyInitiatedCooperativeClosure)
		| Some(ClosureReason::LocallyCoopClosedUnfundedChannel) => events::ChannelClosureInitiator::Local,
		Some(ClosureReason::CounterpartyForceClosed { .. })
		| Some(ClosureReason::CounterpartyInitiatedCooperativeClosure)
		| Some(ClosureReason::CounterpartyCoopClosedUnfundedChannel) => {
			events::ChannelClosureInitiator::Remote
		},
		Some(_) => events::ChannelClosureInitiator::Unknown,
		None => events::ChannelClosureInitiator::Unspecified,
	}
}

fn closure_reason_to_proto(reason: &ClosureReason) -> events::ChannelStateChangeReason {
	events::ChannelStateChangeReason {
		kind: closure_reason_kind(reason).into(),
		message: reason.to_string(),
		details: closure_reason_details(reason),
	}
}

fn closure_reason_kind(reason: &ClosureReason) -> events::ChannelStateChangeReasonKind {
	match reason {
		ClosureReason::CounterpartyForceClosed { .. } => {
			events::ChannelStateChangeReasonKind::CounterpartyForceClosed
		},
		ClosureReason::HolderForceClosed { .. } => {
			events::ChannelStateChangeReasonKind::HolderForceClosed
		},
		ClosureReason::LegacyCooperativeClosure => {
			events::ChannelStateChangeReasonKind::LegacyCooperativeClosure
		},
		ClosureReason::CounterpartyInitiatedCooperativeClosure => {
			events::ChannelStateChangeReasonKind::CounterpartyInitiatedCooperativeClosure
		},
		ClosureReason::LocallyInitiatedCooperativeClosure => {
			events::ChannelStateChangeReasonKind::LocallyInitiatedCooperativeClosure
		},
		ClosureReason::CommitmentTxConfirmed => {
			events::ChannelStateChangeReasonKind::CommitmentTxConfirmed
		},
		ClosureReason::FundingTimedOut => events::ChannelStateChangeReasonKind::FundingTimedOut,
		ClosureReason::ProcessingError { .. } => {
			events::ChannelStateChangeReasonKind::ProcessingError
		},
		ClosureReason::DisconnectedPeer => events::ChannelStateChangeReasonKind::DisconnectedPeer,
		ClosureReason::OutdatedChannelManager => {
			events::ChannelStateChangeReasonKind::OutdatedChannelManager
		},
		ClosureReason::CounterpartyCoopClosedUnfundedChannel => {
			events::ChannelStateChangeReasonKind::CounterpartyCoopClosedUnfundedChannel
		},
		ClosureReason::LocallyCoopClosedUnfundedChannel => {
			events::ChannelStateChangeReasonKind::LocallyCoopClosedUnfundedChannel
		},
		ClosureReason::FundingBatchClosure => {
			events::ChannelStateChangeReasonKind::FundingBatchClosure
		},
		ClosureReason::HTLCsTimedOut { .. } => events::ChannelStateChangeReasonKind::HtlcsTimedOut,
		ClosureReason::PeerFeerateTooLow { .. } => {
			events::ChannelStateChangeReasonKind::PeerFeerateTooLow
		},
	}
}

fn closure_reason_details(
	reason: &ClosureReason,
) -> Option<events::channel_state_change_reason::Details> {
	use events::channel_state_change_reason::Details;

	match reason {
		ClosureReason::CounterpartyForceClosed { peer_msg } => {
			Some(Details::CounterpartyForceClosed(events::CounterpartyForceClosedDetails {
				peer_msg: peer_msg.to_string(),
			}))
		},
		ClosureReason::HolderForceClosed {
			broadcasted_latest_txn,
			message: force_close_message,
		} => Some(Details::HolderForceClosed(events::HolderForceClosedDetails {
			broadcasted_latest_txn: *broadcasted_latest_txn,
			message: force_close_message.clone(),
		})),
		ClosureReason::ProcessingError { err } => {
			Some(Details::ProcessingError(events::ProcessingErrorDetails { err: err.clone() }))
		},
		ClosureReason::HTLCsTimedOut { payment_hash } => {
			Some(Details::HtlcsTimedOut(events::HtlcsTimedOutDetails {
				payment_hash: payment_hash.map(|hash| hash.to_string()),
			}))
		},
		ClosureReason::PeerFeerateTooLow {
			peer_feerate_sat_per_kw,
			required_feerate_sat_per_kw,
		} => Some(Details::PeerFeerateTooLow(events::PeerFeerateTooLowDetails {
			peer_feerate_sat_per_kw: *peer_feerate_sat_per_kw,
			required_feerate_sat_per_kw: *required_feerate_sat_per_kw,
		})),
		_ => None,
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

		debug!("Generated new API key at {}", api_key_path.display());
		Ok(key_bytes.to_lower_hex_string())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use ldk_server_grpc::events::channel_state_change_reason::Details;

	#[test]
	fn test_is_channel_open_failure_classification() {
		assert!(is_channel_open_failure(Some(&ClosureReason::FundingTimedOut)));
		assert!(is_channel_open_failure(Some(&ClosureReason::DisconnectedPeer)));
		assert!(is_channel_open_failure(Some(&ClosureReason::FundingBatchClosure)));
		assert!(is_channel_open_failure(Some(
			&ClosureReason::CounterpartyCoopClosedUnfundedChannel,
		)));
		assert!(is_channel_open_failure(Some(&ClosureReason::LocallyCoopClosedUnfundedChannel,)));

		assert!(!is_channel_open_failure(Some(&ClosureReason::CommitmentTxConfirmed)));
		assert!(!is_channel_open_failure(None));
	}

	#[test]
	fn test_closure_initiator_mapping() {
		assert_eq!(
			closure_initiator_from_reason(Some(&ClosureReason::HolderForceClosed {
				broadcasted_latest_txn: Some(true),
				message: "local close".to_string(),
			})),
			events::ChannelClosureInitiator::Local
		);
		assert_eq!(
			closure_initiator_from_reason(
				Some(&ClosureReason::LocallyInitiatedCooperativeClosure,)
			),
			events::ChannelClosureInitiator::Local
		);

		assert_eq!(
			closure_initiator_from_reason(Some(
				&ClosureReason::CounterpartyInitiatedCooperativeClosure,
			)),
			events::ChannelClosureInitiator::Remote
		);
		assert_eq!(
			closure_initiator_from_reason(Some(
				&ClosureReason::CounterpartyCoopClosedUnfundedChannel,
			)),
			events::ChannelClosureInitiator::Remote
		);

		assert_eq!(
			closure_initiator_from_reason(Some(&ClosureReason::CommitmentTxConfirmed)),
			events::ChannelClosureInitiator::Unknown
		);
		assert_eq!(
			closure_initiator_from_reason(None),
			events::ChannelClosureInitiator::Unspecified
		);
	}

	#[test]
	fn test_closure_reason_to_proto_holder_force_closed_details() {
		let proto = closure_reason_to_proto(&ClosureReason::HolderForceClosed {
			broadcasted_latest_txn: Some(false),
			message: "manual force close".to_string(),
		});

		assert_eq!(proto.kind, events::ChannelStateChangeReasonKind::HolderForceClosed as i32);
		assert!(proto.message.contains("manual force close"));
		match proto.details {
			Some(Details::HolderForceClosed(details)) => {
				assert_eq!(details.broadcasted_latest_txn, Some(false));
				assert_eq!(details.message, "manual force close");
			},
			other => panic!("expected HolderForceClosed details, got {other:?}"),
		}
	}

	#[test]
	fn test_closure_reason_to_proto_peer_feerate_details() {
		let proto = closure_reason_to_proto(&ClosureReason::PeerFeerateTooLow {
			peer_feerate_sat_per_kw: 100,
			required_feerate_sat_per_kw: 250,
		});

		assert_eq!(proto.kind, events::ChannelStateChangeReasonKind::PeerFeerateTooLow as i32);
		match proto.details {
			Some(Details::PeerFeerateTooLow(details)) => {
				assert_eq!(details.peer_feerate_sat_per_kw, 100);
				assert_eq!(details.required_feerate_sat_per_kw, 250);
			},
			other => panic!("expected PeerFeerateTooLow details, got {other:?}"),
		}
	}

	#[test]
	fn test_closure_reason_to_proto_without_details() {
		let proto = closure_reason_to_proto(&ClosureReason::FundingTimedOut);
		assert_eq!(proto.kind, events::ChannelStateChangeReasonKind::FundingTimedOut as i32);
		assert!(proto.details.is_none());
	}
}
