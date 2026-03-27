// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use hex::FromHex;
use ldk_node::lightning_types::payment::PaymentHash;
use ldk_server_protos::api::{Bolt11ReceiveForHashRequest, Bolt11ReceiveForHashResponse};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;
use crate::util::proto_adapter::proto_to_bolt11_description;
use std::sync::Arc;

pub(crate) async fn handle_bolt11_receive_for_hash_request(
	context: Arc<Context>, request: Bolt11ReceiveForHashRequest,
) -> Result<Bolt11ReceiveForHashResponse, LdkServerError> {
	let description = proto_to_bolt11_description(request.description)?;
	let hash_bytes = <[u8; 32]>::from_hex(&request.payment_hash).map_err(|_| {
		LdkServerError::new(
			InvalidRequestError,
			"Invalid payment_hash, must be a 32-byte hex string.".to_string(),
		)
	})?;
	let payment_hash = PaymentHash(hash_bytes);

	let invoice = match request.amount_msat {
		Some(amount_msat) => context.node.bolt11_payment().receive_for_hash(
			amount_msat,
			&description,
			request.expiry_secs,
			payment_hash,
		)?,
		None => context.node.bolt11_payment().receive_variable_amount_for_hash(
			&description,
			request.expiry_secs,
			payment_hash,
		)?,
	};

	Ok(Bolt11ReceiveForHashResponse { invoice: invoice.to_string() })
}
