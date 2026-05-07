// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::fs;

use hex::DisplayHex;
use ldk_server_client::client::LdkServerClient;
use ldk_server_client::error::LdkServerError;
use ldk_server_grpc::api::{GetNodeInfoRequest, GetNodeInfoResponse};

use crate::config::DaemonConfig;

/// Thin wrapper around `LdkServerClient` that loads its credentials from the
/// paths declared in the gateway config.
///
/// Subsequent PRs (read-only tools, write tools, events) will hang per-RPC
/// methods off this type or call into `inner` directly.
pub struct DaemonClient {
	inner: LdkServerClient,
}

impl DaemonClient {
	/// Loads the daemon's API key and TLS certificate from disk and constructs
	/// an authenticated client.
	pub fn new(config: &DaemonConfig) -> Result<Self, String> {
		let api_key_bytes = fs::read(&config.api_key_path).map_err(|e| {
			format!("Failed to read daemon api_key from '{}': {e}", config.api_key_path.display())
		})?;
		if api_key_bytes.is_empty() {
			return Err(format!(
				"Daemon api_key file '{}' is empty",
				config.api_key_path.display()
			));
		}
		let api_key = api_key_bytes.to_lower_hex_string();

		let cert_pem = fs::read(&config.tls_cert_path).map_err(|e| {
			format!("Failed to read daemon TLS cert from '{}': {e}", config.tls_cert_path.display())
		})?;

		let inner = LdkServerClient::new(config.address.clone(), api_key, &cert_pem)?;
		Ok(Self { inner })
	}

	/// Calls `GetNodeInfo` against the daemon, used at boot to verify the
	/// gateway can reach and authenticate to the upstream node.
	pub async fn get_node_info(&self) -> Result<GetNodeInfoResponse, LdkServerError> {
		self.inner.get_node_info(GetNodeInfoRequest {}).await
	}
}
