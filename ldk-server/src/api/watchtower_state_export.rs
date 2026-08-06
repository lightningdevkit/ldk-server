// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::Arc;

use hex::DisplayHex;
use ldk_node::ChannelDetails;
use ldk_server_grpc::api::{WatchtowerStateExportRequest, WatchtowerStateExportResponse};
use ldk_server_grpc::types::{JusticeTransaction, OutPoint, WatchtowerChannelState};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;

pub(crate) async fn handle_watchtower_state_export_request(
	context: Arc<Context>, request: WatchtowerStateExportRequest,
) -> Result<WatchtowerStateExportResponse, LdkServerError> {
	if !context.watchtower_export_enabled {
		return Err(LdkServerError::new(
			InvalidRequestError,
			"Watchtower state export is disabled. Set `watchtower.export_enabled = true` in the server configuration to enable it.",
		));
	}

	let channels = context.node.list_channels();
	let channels = match &request.user_channel_id {
		Some(user_channel_id) => {
			let filtered: Vec<ChannelDetails> = channels
				.into_iter()
				.filter(|channel| channel.user_channel_id.0.to_string() == *user_channel_id)
				.collect();
			if filtered.is_empty() {
				return Err(LdkServerError::new(
					InvalidRequestError,
					format!("Unknown channel with user_channel_id {}", user_channel_id),
				));
			}
			filtered
		},
		None => channels,
	};

	let mut channel_states = Vec::new();
	for channel in &channels {
		if let Some(state) = export_channel_state(&context, channel)? {
			channel_states.push(state);
		}
	}

	Ok(WatchtowerStateExportResponse { channel_states })
}

/// Exports the watchtower-relevant state of a single channel, or `None` if the channel
/// has no negotiated funding output (and therefore no monitor state) yet.
fn export_channel_state(
	context: &Context, channel: &ChannelDetails,
) -> Result<Option<WatchtowerChannelState>, LdkServerError> {
	let funding_txo = match channel.funding_txo {
		Some(funding_txo) => funding_txo,
		None => return Ok(None),
	};

	let justice_transactions = latest_justice_transactions(context, channel)?;

	Ok(Some(WatchtowerChannelState {
		funding_txo: Some(OutPoint { txid: funding_txo.txid.to_string(), vout: funding_txo.vout }),
		user_channel_id: channel.user_channel_id.0.to_string(),
		channel_id: channel.channel_id.0.to_lower_hex_string(),
		counterparty_node_id: channel.counterparty_node_id.to_string(),
		to_self_delay: counterparty_to_self_delay(channel),
		justice_transactions,
	}))
}

/// Returns the `to_self_delay` (in blocks) encumbering the counterparty's `to_local`
/// output, i.e., the delay our justice transactions bypass via the revocation path.
///
/// TODO: Once the ldk-node `feat/expose-chain-monitor` API is available, derive this
/// from the channel's `ChannelMonitor` state, which is authoritative. Until then we
/// fall back to `force_close_spend_delay` from `ChannelDetails`, which is the
/// counterparty's `to_self_delay` as negotiated for *our* commitment transaction; for
/// typical configurations both sides use the same value, but this may diverge if the
/// counterparty configured a different delay for us than we did for them.
fn counterparty_to_self_delay(channel: &ChannelDetails) -> u32 {
	channel.force_close_spend_delay.unwrap_or(0) as u32
}

/// Exports the latest counterparty commitment transaction(s) of the channel - the
/// watchtower locator source - and, where possible, the justice transactions claiming
/// their `to_local` outputs.
///
/// What is wired today: for the newest monitor update carrying counterparty
/// commitments (or the initial commitment if no update carries one), the commitment
/// txid and commitment number are exported.
///
/// TODO(justice-tx): detect the `to_local` output index and value, build the unsigned
/// justice transaction paying a fresh address of this node's on-chain wallet with a
/// conservative fee, and sign it via `Node::sign_to_local_justice_tx` once the state
/// has been revoked by a newer one. Until then `to_local_value_sats` is 0,
/// `justice_tx` is empty, and `signed` is false: clients can derive locators but
/// cannot finalize appointments yet.
fn latest_justice_transactions(
	context: &Context, channel: &ChannelDetails,
) -> Result<Vec<JusticeTransaction>, LdkServerError> {
	let channel_id = channel.channel_id;

	let mut commitment_txs = Vec::new();
	let updates = context.node.channel_monitor_updates(channel_id).map_err(LdkServerError::from)?;
	// Walk updates newest-first: not every update carries new counterparty commitments.
	for update in updates.iter().rev() {
		let txs = context
			.node
			.counterparty_commitment_txs_from_update(channel_id, update.clone())
			.map_err(LdkServerError::from)?;
		if !txs.is_empty() {
			commitment_txs = txs;
			break;
		}
	}
	if commitment_txs.is_empty() {
		if let Some(initial) = context
			.node
			.initial_counterparty_commitment_tx(channel_id)
			.map_err(LdkServerError::from)?
		{
			commitment_txs.push(initial);
		}
	}

	Ok(commitment_txs
		.into_iter()
		.map(|tx| JusticeTransaction {
			commitment_txid: tx.trust().txid().to_string(),
			commitment_number: tx.commitment_number(),
			// See TODO(justice-tx) above.
			to_local_value_sats: 0,
			justice_tx: bytes::Bytes::new(),
			signed: false,
		})
		.collect())
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use hex::DisplayHex;
	use ldk_node::bitcoin::Network;
	use ldk_node::entropy::NodeEntropy;
	use ldk_node::Builder;

	use super::*;
	use crate::io::persist::sqlite_store::SqliteStore;

	fn random_storage_path() -> PathBuf {
		let mut temp_path = std::env::temp_dir();
		let mut bytes = [0u8; 8];
		getrandom::getrandom(&mut bytes).expect("Failed to generate random bytes");
		temp_path.push(bytes.to_lower_hex_string());
		temp_path
	}

	fn build_test_context(watchtower_export_enabled: bool) -> Arc<Context> {
		let storage_dir = random_storage_path();

		let mut node_config = ldk_node::config::Config::default();
		node_config.network = Network::Regtest;
		node_config.storage_dir_path = storage_dir.to_str().unwrap().to_string();

		let mut builder = Builder::from_config(node_config);
		// The chain source is never queried as the node is not started.
		builder.set_chain_source_esplora("http://127.0.0.1:1".to_string(), None);
		let node_entropy =
			NodeEntropy::from_seed_path(storage_dir.join("seed").to_str().unwrap().to_string())
				.unwrap();
		let node = builder.build(node_entropy).unwrap();

		let paginated_kv_store = SqliteStore::new(
			storage_dir.join("paginated_kv"),
			Some("test_db".to_string()),
			Some("test_table".to_string()),
		)
		.unwrap();

		Arc::new(Context {
			node: node.into(),
			paginated_kv_store: Arc::new(paginated_kv_store),
			watchtower_export_enabled,
		})
	}

	#[test]
	fn export_disabled_returns_error() {
		let context = build_test_context(false);

		let request = WatchtowerStateExportRequest { user_channel_id: None };
		let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
		// Keep a second strong ref outside the async context: the handler's Arc is
		// dropped inside the runtime, but the node must only be dropped after the
		// runtime is gone (dropping a runtime within an async context panics).
		let result = rt.block_on(handle_watchtower_state_export_request(
			std::sync::Arc::clone(&context),
			request,
		));
		drop(rt);

		let err = result.unwrap_err();
		assert_eq!(err.error_code, InvalidRequestError);
		assert!(err.message.contains("disabled"));
	}
}
