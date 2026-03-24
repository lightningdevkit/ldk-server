// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use ldk_node::bitcoin::hashes::Hash;
use ldk_node::payment::UnifiedPaymentResult;
use ldk_server_json_models::api::{UnifiedSendRequest, UnifiedSendResponse};
use tokio::runtime::Handle;

use crate::api::build_route_parameters_config_from_model;
use crate::api::error::LdkServerError;
use crate::service::Context;
use crate::util::adapter::to_display_bytes;

pub(crate) fn handle_unified_send_request(
	context: Context, request: UnifiedSendRequest,
) -> Result<UnifiedSendResponse, LdkServerError> {
	let route_parameters = build_route_parameters_config_from_model(request.route_parameters)?;

	let result = tokio::task::block_in_place(|| {
		Handle::current().block_on(context.node.unified_payment().send(
			&request.uri,
			request.amount_msat,
			route_parameters,
		))
	})?;

	Ok(match result {
		UnifiedPaymentResult::Onchain { txid } => {
			let payment_id = txid.to_byte_array();
			UnifiedSendResponse::Onchain {
				txid: to_display_bytes(txid.to_byte_array()),
				payment_id,
			}
		},
		UnifiedPaymentResult::Bolt11 { payment_id } => {
			UnifiedSendResponse::Bolt11 { payment_id: payment_id.0 }
		},
		UnifiedPaymentResult::Bolt12 { payment_id } => {
			UnifiedSendResponse::Bolt12 { payment_id: payment_id.0 }
		},
	})
}
