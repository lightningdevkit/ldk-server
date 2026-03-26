// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use hex::FromHex;
use ldk_node::bitcoin::hashes::{sha256, Hash};
use ldk_node::lightning_types::payment::{PaymentHash, PaymentPreimage};
use ldk_server_protos::api::{Bolt11ClaimForHashRequest, Bolt11ClaimForHashResponse};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::grpc_service::Context;

pub(crate) fn handle_bolt11_claim_for_hash_request(
	context: &Context, request: Bolt11ClaimForHashRequest,
) -> Result<Bolt11ClaimForHashResponse, LdkServerError> {
	let preimage_bytes = <[u8; 32]>::from_hex(&request.preimage).map_err(|_| {
		LdkServerError::new(
			InvalidRequestError,
			"Invalid preimage, must be a 32-byte hex string.".to_string(),
		)
	})?;
	let preimage = PaymentPreimage(preimage_bytes);

	let payment_hash = if let Some(hash_hex) = &request.payment_hash {
		let hash_bytes = <[u8; 32]>::from_hex(hash_hex).map_err(|_| {
			LdkServerError::new(
				InvalidRequestError,
				"Invalid payment_hash, must be a 32-byte hex string.".to_string(),
			)
		})?;
		PaymentHash(hash_bytes)
	} else {
		PaymentHash(sha256::Hash::hash(&preimage.0).to_byte_array())
	};

	let claimable_amount_msat = request.claimable_amount_msat.unwrap_or(u64::MAX);

	context.node.bolt11_payment().claim_for_hash(payment_hash, claimable_amount_msat, preimage)?;

	Ok(Bolt11ClaimForHashResponse {})
}
