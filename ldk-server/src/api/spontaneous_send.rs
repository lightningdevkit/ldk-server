// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::str::FromStr;

use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_server_protos::api::{SpontaneousSendRequest, SpontaneousSendResponse};

use crate::api::build_route_parameters_config_from_proto;
use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;

pub(crate) fn handle_spontaneous_send_request(
	context: Context, request: SpontaneousSendRequest,
) -> Result<SpontaneousSendResponse, LdkServerError> {
	let node_id = PublicKey::from_str(&request.node_id).map_err(|_| {
		LdkServerError::new(InvalidRequestError, "Invalid node_id provided.".to_string())
	})?;

	let route_parameters = build_route_parameters_config_from_proto(request.route_parameters)?;

	let payment_id =
		context.node.spontaneous_payment().send(request.amount_msat, node_id, route_parameters)?;

	let response = SpontaneousSendResponse { payment_id: payment_id.to_string() };
	Ok(response)
}
