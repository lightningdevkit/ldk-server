// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

mod config;
mod daemon;
mod util;
mod web;

use std::fs;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use hyper_util::service::TowerToHyperService;
use log::{debug, error, info};
use tokio::net::TcpListener;
use tokio::select;
use tokio::signal::unix::SignalKind;

use crate::config::{load_config, ArgsConfig, Config};
use crate::daemon::client::DaemonClient;
use crate::util::logger::GatewayLogger;
use crate::util::tls::get_or_generate_tls_config;
use crate::web::routes::build_router;

fn main() -> ExitCode {
	let args = ArgsConfig::parse();

	let config = match load_config(&args) {
		Ok(c) => c,
		Err(e) => {
			eprintln!("Invalid configuration: {e}");
			return ExitCode::from(1);
		},
	};

	if let Err(e) = fs::create_dir_all(&config.storage_dir) {
		eprintln!("Failed to create storage_dir '{}': {e}", config.storage_dir.display());
		return ExitCode::from(1);
	}

	let logger = match GatewayLogger::init(config.log_level, &config.log_file_path) {
		Ok(l) => l,
		Err(e) => {
			eprintln!("Failed to initialize logger: {e}");
			return ExitCode::from(1);
		},
	};

	let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
		Ok(rt) => Arc::new(rt),
		Err(e) => {
			error!("Failed to set up tokio runtime: {e}");
			return ExitCode::from(1);
		},
	};

	runtime.block_on(async {
		match run(config, logger).await {
			Ok(()) => ExitCode::from(0),
			Err(e) => {
				error!("Fatal: {e}");
				ExitCode::from(1)
			},
		}
	})
}

async fn run(config: Config, logger: Arc<GatewayLogger>) -> Result<(), String> {
	let storage_dir_str = config
		.storage_dir
		.to_str()
		.ok_or_else(|| format!("storage_dir contains non-UTF-8 bytes: {:?}", config.storage_dir))?;

	let server_config = get_or_generate_tls_config(config.tls_config.clone(), storage_dir_str)?;
	let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

	let daemon_client = DaemonClient::new(&config.daemon)?;
	match daemon_client.get_node_info().await {
		Ok(info) => {
			let alias = info.node_alias.as_deref().unwrap_or("<unset>");
			info!("daemon connection ok (node_id='{}', alias='{}')", info.node_id, alias);
		},
		Err(e) => {
			return Err(format!("Daemon health check failed: {e:?}"));
		},
	}

	let router = build_router();

	let listener = TcpListener::bind(config.listen_addr)
		.await
		.map_err(|e| format!("Failed to bind {}: {e}", config.listen_addr))?;
	info!("gateway listening on https://{}", config.listen_addr);

	let mut sighup_stream = tokio::signal::unix::signal(SignalKind::hangup())
		.map_err(|e| format!("Failed to register SIGHUP handler: {e}"))?;
	let mut sigterm_stream = tokio::signal::unix::signal(SignalKind::terminate())
		.map_err(|e| format!("Failed to register SIGTERM handler: {e}"))?;

	loop {
		select! {
			res = listener.accept() => {
				match res {
					Ok((tcp, peer)) => {
						let acceptor = tls_acceptor.clone();
						let svc = TowerToHyperService::new(router.clone());
						tokio::spawn(async move {
							let tls = match acceptor.accept(tcp).await {
								Ok(s) => s,
								Err(e) => {
									debug!("TLS handshake failed for {peer}: {e}");
									return;
								}
							};
							let io = TokioIo::new(tls);
							if let Err(e) = auto::Builder::new(TokioExecutor::new())
								.serve_connection(io, svc)
								.await
							{
								debug!("Connection error from {peer}: {e}");
							}
						});
					},
					Err(e) => error!("Failed to accept connection: {e}"),
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
			_ = sighup_stream.recv() => {
				if let Err(e) = logger.reopen() {
					error!("Failed to reopen log file on SIGHUP: {e}");
				}
			}
		}
	}

	info!("Shutdown complete..");
	Ok(())
}
