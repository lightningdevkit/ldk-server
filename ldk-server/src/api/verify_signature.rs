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
use ldk_server_protos::api::{VerifySignatureRequest, VerifySignatureResponse};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;

pub(crate) fn handle_verify_signature_request(
	context: Context, request: VerifySignatureRequest,
) -> Result<VerifySignatureResponse, LdkServerError> {
	let public_key = PublicKey::from_str(&request.public_key).map_err(|_| {
		LdkServerError::new(InvalidRequestError, "Invalid public_key provided.".to_string())
	})?;

	let valid = context.node.verify_signature(&request.message, &request.signature, &public_key);

	let response = VerifySignatureResponse { valid };
	Ok(response)
}
