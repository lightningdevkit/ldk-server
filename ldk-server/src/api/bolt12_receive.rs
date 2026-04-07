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
use ldk_server_grpc::api::{Bolt12ReceiveRequest, Bolt12ReceiveResponse};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;

pub(crate) async fn handle_bolt12_receive_request(
	context: Arc<Context>, request: Bolt12ReceiveRequest,
) -> Result<Bolt12ReceiveResponse, LdkServerError> {
	let offer =
		match (request.amount_msat, request.quantity) {
			(Some(amount_msat), quantity) => context.node.bolt12_payment().receive(
				amount_msat,
				&request.description,
				request.expiry_secs,
				quantity,
			)?,
			(None, Some(_)) => return Err(LdkServerError::new(
				InvalidRequestError,
				"quantity can only be set for fixed-amount offers (amount_msat must be provided)",
			)),
			(None, None) => context
				.node
				.bolt12_payment()
				.receive_variable_amount(&request.description, request.expiry_secs)?,
		};

	let offer_id = offer.id().0.to_lower_hex_string();
	let response = Bolt12ReceiveResponse { offer: offer.to_string(), offer_id };
	Ok(response)
}
