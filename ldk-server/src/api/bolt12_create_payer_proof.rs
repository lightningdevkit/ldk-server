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
use ldk_node::lightning::offers::invoice::Bolt12Invoice;
use ldk_node::lightning_types::payment::PaymentPreimage;
use ldk_node::payment::PayerProofOptions;
use ldk_server_grpc::api::{Bolt12CreatePayerProofRequest, Bolt12CreatePayerProofResponse};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;

pub(crate) async fn handle_bolt12_create_payer_proof_request(
	context: Arc<Context>, request: Bolt12CreatePayerProofRequest,
) -> Result<Bolt12CreatePayerProofResponse, LdkServerError> {
	let payment_id = crate::api::parse_payment_id(&request.payment_id)?;

	let preimage_bytes = <[u8; 32]>::from_hex(&request.payment_preimage).map_err(|_| {
		LdkServerError::new(
			InvalidRequestError,
			"Invalid payment_preimage, must be a 32-byte hex string.".to_string(),
		)
	})?;
	let payment_preimage = PaymentPreimage(preimage_bytes);

	let invoice_bytes = Vec::<u8>::from_hex(&request.invoice).map_err(|_| {
		LdkServerError::new(
			InvalidRequestError,
			"Invalid invoice, must be a hex-encoded BOLT 12 invoice.".to_string(),
		)
	})?;
	let invoice =
		Bolt12Invoice::try_from(invoice_bytes).map_err(|_| ldk_node::NodeError::InvalidInvoice)?;

	let options = request.options.map(|options| PayerProofOptions {
		note: options.note,
		include_offer_description: options.include_offer_description,
		include_offer_issuer: options.include_offer_issuer,
		include_invoice_amount: options.include_invoice_amount,
		include_invoice_created_at: options.include_invoice_created_at,
		extra_tlv_types: options.extra_tlv_types,
	});

	let payer_proof = context.node.bolt12_payment().create_payer_proof(
		payment_id,
		payment_preimage,
		&invoice,
		options,
	)?;

	Ok(Bolt12CreatePayerProofResponse { payer_proof: payer_proof.to_string() })
}
