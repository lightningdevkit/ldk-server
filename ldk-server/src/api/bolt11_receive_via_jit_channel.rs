// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::Arc;

use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use hex::FromHex;
use ldk_node::lightning_types::payment::PaymentHash;
use ldk_server_grpc::api::{
	Bolt11ReceiveVariableAmountViaJitChannelForHashRequest,
	Bolt11ReceiveVariableAmountViaJitChannelForHashResponse,
	Bolt11ReceiveVariableAmountViaJitChannelRequest,
	Bolt11ReceiveVariableAmountViaJitChannelResponse, Bolt11ReceiveViaJitChannelForHashRequest,
	Bolt11ReceiveViaJitChannelForHashResponse, Bolt11ReceiveViaJitChannelRequest,
	Bolt11ReceiveViaJitChannelResponse,
};

use crate::api::error::LdkServerError;
use crate::service::Context;
use crate::util::proto_adapter::proto_to_bolt11_description;

pub(crate) async fn handle_bolt11_receive_via_jit_channel_request(
	context: Arc<Context>, request: Bolt11ReceiveViaJitChannelRequest,
) -> Result<Bolt11ReceiveViaJitChannelResponse, LdkServerError> {
	let description = proto_to_bolt11_description(request.description)?;
	let invoice = context.node.bolt11_payment().receive_via_jit_channel(
		request.amount_msat,
		&description,
		request.expiry_secs,
		request.max_total_lsp_fee_limit_msat,
	)?;

	Ok(Bolt11ReceiveViaJitChannelResponse { invoice: invoice.to_string() })
}

pub(crate) async fn handle_bolt11_receive_variable_amount_via_jit_channel_request(
	context: Arc<Context>, request: Bolt11ReceiveVariableAmountViaJitChannelRequest,
) -> Result<Bolt11ReceiveVariableAmountViaJitChannelResponse, LdkServerError> {
	let description = proto_to_bolt11_description(request.description)?;
	let invoice = context.node.bolt11_payment().receive_variable_amount_via_jit_channel(
		&description,
		request.expiry_secs,
		request.max_proportional_lsp_fee_limit_ppm_msat,
	)?;

	Ok(Bolt11ReceiveVariableAmountViaJitChannelResponse { invoice: invoice.to_string() })
}

pub(crate) async fn handle_bolt11_receive_via_jit_channel_for_hash_request(
	context: Arc<Context>, request: Bolt11ReceiveViaJitChannelForHashRequest,
) -> Result<Bolt11ReceiveViaJitChannelForHashResponse, LdkServerError> {
	let description = proto_to_bolt11_description(request.description)?;
	let hash_bytes = <[u8; 32]>::from_hex(&request.payment_hash).map_err(|_| {
		LdkServerError::new(
			InvalidRequestError,
			"Invalid payment_hash, must be a 32-byte hex string.".to_string(),
		)
	})?;
	let payment_hash = PaymentHash(hash_bytes);
	let invoice = context.node.bolt11_payment().receive_via_jit_channel_for_hash(
		request.amount_msat,
		&description,
		request.expiry_secs,
		request.max_total_lsp_fee_limit_msat,
		payment_hash,
	)?;

	Ok(Bolt11ReceiveViaJitChannelForHashResponse { invoice: invoice.to_string() })
}

pub(crate) async fn handle_bolt11_receive_variable_amount_via_jit_channel_for_hash_request(
	context: Arc<Context>, request: Bolt11ReceiveVariableAmountViaJitChannelForHashRequest,
) -> Result<Bolt11ReceiveVariableAmountViaJitChannelForHashResponse, LdkServerError> {
	let description = proto_to_bolt11_description(request.description)?;
	let hash_bytes = <[u8; 32]>::from_hex(&request.payment_hash).map_err(|_| {
		LdkServerError::new(
			InvalidRequestError,
			"Invalid payment_hash, must be a 32-byte hex string.".to_string(),
		)
	})?;
	let payment_hash = PaymentHash(hash_bytes);
	let invoice = context.node.bolt11_payment().receive_variable_amount_via_jit_channel_for_hash(
		&description,
		request.expiry_secs,
		request.max_proportional_lsp_fee_limit_ppm_msat,
		payment_hash,
	)?;
	Ok(Bolt11ReceiveVariableAmountViaJitChannelForHashResponse { invoice: invoice.to_string() })
}
