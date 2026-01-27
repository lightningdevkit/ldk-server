// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::str::FromStr;

use ldk_node::lightning_invoice::Bolt11Invoice;
use ldk_server_protos::api::{Bolt11SendRequest, Bolt11SendResponse};

use crate::api::build_route_parameters_config_from_proto;
use crate::api::error::LdkServerError;
use crate::service::Context;

pub(crate) fn handle_bolt11_send_request(
	context: Context, request: Bolt11SendRequest,
) -> Result<Bolt11SendResponse, LdkServerError> {
	let invoice = Bolt11Invoice::from_str(request.invoice.as_str())
		.map_err(|_| ldk_node::NodeError::InvalidInvoice)?;

	let route_parameters = build_route_parameters_config_from_proto(request.route_parameters)?;

	let payment_id = match request.amount_msat {
		None => context.node.bolt11_payment().send(&invoice, route_parameters),
		Some(amount_msat) => {
			context.node.bolt11_payment().send_using_amount(&invoice, amount_msat, route_parameters)
		},
	}?;

	let response = Bolt11SendResponse { payment_id: payment_id.to_string() };
	Ok(response)
}
