// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use ldk_node::payment::UnifiedPaymentResult;
use ldk_server_json_models::api::{
	UnifiedSendPaymentResult, UnifiedSendRequest, UnifiedSendResponse,
};
use tokio::runtime::Handle;

use crate::api::build_route_parameters_config_from_model;
use crate::api::error::LdkServerError;
use crate::service::Context;

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

	let payment_result = match result {
		UnifiedPaymentResult::Onchain { txid } => UnifiedSendPaymentResult::Txid(txid.to_string()),
		UnifiedPaymentResult::Bolt11 { payment_id } => {
			UnifiedSendPaymentResult::Bolt11PaymentId(payment_id.to_string())
		},
		UnifiedPaymentResult::Bolt12 { payment_id } => {
			UnifiedSendPaymentResult::Bolt12PaymentId(payment_id.to_string())
		},
	};

	Ok(UnifiedSendResponse { payment_result: Some(payment_result) })
}
