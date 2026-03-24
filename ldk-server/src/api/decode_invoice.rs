// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::str::FromStr;

use hex::prelude::*;
use ldk_node::lightning_invoice::Bolt11Invoice;
use ldk_node::lightning_types::features::Bolt11InvoiceFeatures;
use ldk_server_json_models::api::{DecodeInvoiceRequest, DecodeInvoiceResponse};
use ldk_server_json_models::types::{Bolt11HopHint, Bolt11RouteHint};

use crate::api::decode_features;
use crate::api::error::LdkServerError;
use crate::service::Context;

pub(crate) fn handle_decode_invoice_request(
	_context: Context, request: DecodeInvoiceRequest,
) -> Result<DecodeInvoiceResponse, LdkServerError> {
	let invoice = Bolt11Invoice::from_str(request.invoice.as_str())
		.map_err(|_| ldk_node::NodeError::InvalidInvoice)?;

	let destination = invoice.get_payee_pub_key().to_string();
	let payment_hash = invoice.payment_hash().0.to_lower_hex_string();
	let amount_msat = invoice.amount_milli_satoshis();
	let timestamp = invoice.duration_since_epoch().as_secs();
	let expiry = invoice.expiry_time().as_secs();
	let min_final_cltv_expiry_delta = invoice.min_final_cltv_expiry_delta();
	let payment_secret = invoice.payment_secret().0.to_lower_hex_string();

	let (description, description_hash) = match invoice.description() {
		ldk_node::lightning_invoice::Bolt11InvoiceDescriptionRef::Direct(desc) => {
			(Some(desc.to_string()), None)
		},
		ldk_node::lightning_invoice::Bolt11InvoiceDescriptionRef::Hash(hash) => {
			(None, Some(hash.0.to_string()))
		},
	};

	let fallback_address = invoice.fallback_addresses().into_iter().next().map(|a| a.to_string());

	let route_hints = invoice
		.route_hints()
		.into_iter()
		.map(|hint| Bolt11RouteHint {
			hop_hints: hint
				.0
				.iter()
				.map(|hop| Bolt11HopHint {
					node_id: hop.src_node_id.to_string(),
					short_channel_id: hop.short_channel_id,
					fee_base_msat: hop.fees.base_msat,
					fee_proportional_millionths: hop.fees.proportional_millionths,
					cltv_expiry_delta: hop.cltv_expiry_delta as u32,
				})
				.collect(),
		})
		.collect();

	let features = invoice
		.features()
		.map(|f| {
			decode_features(f.le_flags(), |bytes| {
				Bolt11InvoiceFeatures::from_le_bytes(bytes).to_string()
			})
		})
		.unwrap_or_default();

	let currency = match invoice.currency() {
		ldk_node::lightning_invoice::Currency::Bitcoin => "bitcoin",
		ldk_node::lightning_invoice::Currency::BitcoinTestnet => "testnet",
		ldk_node::lightning_invoice::Currency::Regtest => "regtest",
		ldk_node::lightning_invoice::Currency::Simnet => "simnet",
		ldk_node::lightning_invoice::Currency::Signet => "signet",
	}
	.to_string();

	let payment_metadata = invoice.payment_metadata().map(|m| m.to_lower_hex_string());

	let is_expired = invoice.is_expired();

	Ok(DecodeInvoiceResponse {
		destination,
		payment_hash,
		amount_msat,
		timestamp,
		expiry,
		description,
		description_hash,
		fallback_address,
		min_final_cltv_expiry_delta,
		payment_secret,
		route_hints,
		features,
		currency,
		payment_metadata,
		is_expired,
	})
}
