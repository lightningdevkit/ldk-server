// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use ldk_server_json_models::api::{
	Bolt11ReceiveVariableAmountViaJitChannelRequest,
	Bolt11ReceiveVariableAmountViaJitChannelResponse, Bolt11ReceiveViaJitChannelRequest,
	Bolt11ReceiveViaJitChannelResponse,
};

use crate::api::error::LdkServerError;
use crate::service::Context;
use crate::util::adapter::bolt11_description_from_model;

pub(crate) fn handle_bolt11_receive_via_jit_channel_request(
	context: Context, request: Bolt11ReceiveViaJitChannelRequest,
) -> Result<Bolt11ReceiveViaJitChannelResponse, LdkServerError> {
	let description = bolt11_description_from_model(request.description)?;
	let invoice = context.node.bolt11_payment().receive_via_jit_channel(
		request.amount_msat,
		&description,
		request.expiry_secs,
		request.max_total_lsp_fee_limit_msat,
	)?;

	let payment_hash = invoice.payment_hash().0;
	let payment_secret = invoice.payment_secret().0;
	Ok(Bolt11ReceiveViaJitChannelResponse {
		invoice: invoice.to_string(),
		payment_hash,
		payment_secret,
	})
}

pub(crate) fn handle_bolt11_receive_variable_amount_via_jit_channel_request(
	context: Context, request: Bolt11ReceiveVariableAmountViaJitChannelRequest,
) -> Result<Bolt11ReceiveVariableAmountViaJitChannelResponse, LdkServerError> {
	let description = bolt11_description_from_model(request.description)?;
	let invoice = context.node.bolt11_payment().receive_variable_amount_via_jit_channel(
		&description,
		request.expiry_secs,
		request.max_proportional_lsp_fee_limit_ppm_msat,
	)?;

	let payment_hash = invoice.payment_hash().0;
	let payment_secret = invoice.payment_secret().0;
	Ok(Bolt11ReceiveVariableAmountViaJitChannelResponse {
		invoice: invoice.to_string(),
		payment_hash,
		payment_secret,
	})
}
