// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::Arc;

use hex::FromHex;
use ldk_node::lightning_types::payment::PaymentHash;
use ldk_server_grpc::api::{Bolt11FailForHashRequest, Bolt11FailForHashResponse};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;

pub(crate) async fn handle_bolt11_fail_for_hash_request(
	context: Arc<Context>, request: Bolt11FailForHashRequest,
) -> Result<Bolt11FailForHashResponse, LdkServerError> {
	let hash_bytes = <[u8; 32]>::from_hex(&request.payment_hash).map_err(|_| {
		LdkServerError::new(
			InvalidRequestError,
			"Invalid payment_hash, must be a 32-byte hex string.".to_string(),
		)
	})?;
	let payment_hash = PaymentHash(hash_bytes);

	context.node.bolt11_payment().fail_for_hash(payment_hash)?;

	Ok(Bolt11FailForHashResponse {})
}
