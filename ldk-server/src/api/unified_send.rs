// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use ldk_node::payment::UnifiedPaymentResult;
use ldk_server_grpc::api::unified_send_response::PaymentResult;
use ldk_server_grpc::api::{UnifiedSendRequest, UnifiedSendResponse};

use crate::api::build_route_parameters_config_from_proto;
use crate::api::error::LdkServerError;
use crate::service::Context;
use std::sync::Arc;

pub(crate) async fn handle_unified_send_request(
	context: Arc<Context>, request: UnifiedSendRequest,
) -> Result<UnifiedSendResponse, LdkServerError> {
	let route_parameters = build_route_parameters_config_from_proto(request.route_parameters)?;

	let result = context
		.node
		.unified_payment()
		.send(&request.uri, request.amount_msat, route_parameters)
		.await?;

	let payment_result = match result {
		UnifiedPaymentResult::Onchain { txid } => PaymentResult::Txid(txid.to_string()),
		UnifiedPaymentResult::Bolt11 { payment_id } => {
			PaymentResult::Bolt11PaymentId(payment_id.to_string())
		},
		UnifiedPaymentResult::Bolt12 { payment_id } => {
			PaymentResult::Bolt12PaymentId(payment_id.to_string())
		},
	};

	Ok(UnifiedSendResponse { payment_result: Some(payment_result) })
}
