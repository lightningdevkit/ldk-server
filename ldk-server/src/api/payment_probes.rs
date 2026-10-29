// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::str::FromStr;
use std::sync::Arc;

use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::lightning_invoice::Bolt11Invoice;
use ldk_server_grpc::api::{
	Bolt11SendProbesRequest, Bolt11SendProbesResponse, SpontaneousSendProbesRequest,
	SpontaneousSendProbesResponse,
};

use crate::api::build_route_parameters_config_from_proto;
use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;

pub(crate) async fn handle_bolt11_send_probes_request(
	context: Arc<Context>, request: Bolt11SendProbesRequest,
) -> Result<Bolt11SendProbesResponse, LdkServerError> {
	let invoice = Bolt11Invoice::from_str(&request.invoice)
		.map_err(|_| ldk_node::NodeError::InvalidInvoice)?;
	let route_parameters = build_route_parameters_config_from_proto(request.route_parameters)?;

	match request.amount_msat {
		Some(amount_msat) => context.node.bolt11_payment().send_probes_using_amount(
			&invoice,
			amount_msat,
			route_parameters,
		)?,
		None => context.node.bolt11_payment().send_probes(&invoice, route_parameters)?,
	};

	Ok(Bolt11SendProbesResponse {})
}

pub(crate) async fn handle_spontaneous_send_probes_request(
	context: Arc<Context>, request: SpontaneousSendProbesRequest,
) -> Result<SpontaneousSendProbesResponse, LdkServerError> {
	let node_id = PublicKey::from_str(&request.node_id)
		.map_err(|_| LdkServerError::new(InvalidRequestError, "Invalid node_id provided."))?;

	context.node.spontaneous_payment().send_probes(request.amount_msat, node_id)?;

	Ok(SpontaneousSendProbesResponse {})
}
