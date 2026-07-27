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

use hex::FromHex;
use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::lightning_types::payment::PaymentPreimage;
use ldk_node::CustomTlvRecord;
use ldk_server_grpc::api::{SpontaneousSendRequest, SpontaneousSendResponse};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::api::{build_route_parameters_config_from_proto, proto_to_node_custom_tlv};
use crate::service::Context;

pub(crate) async fn handle_spontaneous_send_request(
	context: Arc<Context>, request: SpontaneousSendRequest,
) -> Result<SpontaneousSendResponse, LdkServerError> {
	let node_id = PublicKey::from_str(&request.node_id).map_err(|_| {
		LdkServerError::new(InvalidRequestError, "Invalid node_id provided.".to_string())
	})?;

	let route_parameters = build_route_parameters_config_from_proto(request.route_parameters)?;

	let preimage = request
		.preimage
		.map(|p| {
			<[u8; 32]>::from_hex(&p).map(PaymentPreimage).map_err(|_| {
				LdkServerError::new(
					InvalidRequestError,
					"Invalid preimage, must be a 32-byte hex string.".to_string(),
				)
			})
		})
		.transpose()?;

	let custom_tlvs: Option<Vec<CustomTlvRecord>> = if request.custom_tlvs.is_empty() {
		None
	} else {
		Some(request.custom_tlvs.iter().map(proto_to_node_custom_tlv).collect())
	};

	let payment_id = match (preimage, custom_tlvs) {
		(None, None) => context.node.spontaneous_payment().send(
			request.amount_msat,
			node_id,
			route_parameters,
		)?,
		(None, Some(custom_tlvs)) => context.node.spontaneous_payment().send_with_custom_tlvs(
			request.amount_msat,
			node_id,
			route_parameters,
			custom_tlvs,
		)?,
		(Some(preimage), None) => context.node.spontaneous_payment().send_with_preimage(
			request.amount_msat,
			node_id,
			preimage,
			route_parameters,
		)?,
		(Some(preimage), Some(custom_tlvs)) => {
			context.node.spontaneous_payment().send_with_preimage_and_custom_tlvs(
				request.amount_msat,
				node_id,
				custom_tlvs,
				preimage,
				route_parameters,
			)?
		},
	};

	Ok(SpontaneousSendResponse { payment_id: payment_id.to_string() })
}
