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

use hex::prelude::*;
use ldk_node::lightning::offers::invoice::Bolt12Invoice;
use ldk_node::lightning_invoice::Bolt11Invoice;
use ldk_node::lightning_types::features::{Bolt11InvoiceFeatures, Bolt12InvoiceFeatures};
use ldk_server_grpc::api::{DecodeInvoiceRequest, DecodeInvoiceResponse};
use ldk_server_grpc::types::{Bolt11HopHint, Bolt11RouteHint};

use crate::api::error::LdkServerError;
use crate::api::{blinded_path_to_proto, decode_features};
use crate::service::Context;

const INVOICE_KIND_BOLT11: &str = "bolt11";
const INVOICE_KIND_BOLT12: &str = "bolt12";

pub(crate) async fn handle_decode_invoice_request(
	_context: Arc<Context>, request: DecodeInvoiceRequest,
) -> Result<DecodeInvoiceResponse, LdkServerError> {
	decode_invoice(request.invoice.as_str())
}

/// Decodes either a BOLT11 invoice string or a hex-encoded BOLT12 invoice.
fn decode_invoice(invoice: &str) -> Result<DecodeInvoiceResponse, LdkServerError> {
	if let Ok(bolt11_invoice) = Bolt11Invoice::from_str(invoice) {
		return Ok(decode_bolt11_invoice(&bolt11_invoice));
	}

	if let Some(response) = decode_bolt12_invoice(invoice) {
		return Ok(response);
	}

	Err(ldk_node::NodeError::InvalidInvoice.into())
}

/// Attempts to decode `invoice` as a hex-encoded BOLT12 invoice.
///
/// Unlike offers and BOLT11 invoices, a BOLT12 invoice has no human-readable string
/// encoding — it is exchanged as raw bytes — so the input is expected to be hex-encoded.
/// Fields that do not apply to BOLT12 invoices (e.g. `payment_secret`, `route_hints`) are
/// left at their default empty values.
fn decode_bolt12_invoice(invoice: &str) -> Option<DecodeInvoiceResponse> {
	let bytes = Vec::<u8>::from_hex(invoice).ok()?;
	let invoice = Bolt12Invoice::try_from(bytes).ok()?;

	let features = decode_features(invoice.invoice_features().le_flags(), |bytes| {
		Bolt12InvoiceFeatures::from_le_bytes(bytes).to_string()
	});

	let paths = invoice
		.payment_paths()
		.iter()
		.map(|path| {
			blinded_path_to_proto(
				path.introduction_node(),
				path.blinding_point(),
				path.blinded_hops().len(),
			)
		})
		.collect();

	Some(DecodeInvoiceResponse {
		destination: invoice.signing_pubkey().to_string(),
		payment_hash: invoice.payment_hash().0.to_lower_hex_string(),
		amount_msat: Some(invoice.amount_msats()),
		timestamp: invoice.created_at().as_secs(),
		expiry: invoice.relative_expiry().as_secs(),
		description: invoice.description().map(|d| d.to_string()),
		fallback_address: invoice.fallbacks().into_iter().next().map(|a| a.to_string()),
		features,
		is_expired: invoice.is_expired(),
		kind: INVOICE_KIND_BOLT12.to_string(),
		paths,
		..Default::default()
	})
}

fn decode_bolt11_invoice(invoice: &Bolt11Invoice) -> DecodeInvoiceResponse {
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

	DecodeInvoiceResponse {
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
		kind: INVOICE_KIND_BOLT11.to_string(),
		// BOLT11 invoices carry route hints rather than blinded paths.
		paths: Vec::new(),
	}
}

#[cfg(test)]
mod tests {
	use ldk_node::lightning::bitcoin::secp256k1::{Keypair, PublicKey, Secp256k1, SecretKey};
	use ldk_node::lightning::blinded_path::payment::{BlindedPayInfo, BlindedPaymentPath};
	use ldk_node::lightning::blinded_path::BlindedHop;
	use ldk_node::lightning::offers::invoice::UnsignedBolt12Invoice;
	use ldk_node::lightning::offers::refund::RefundBuilder;
	use ldk_node::lightning::types::features::BlindedHopFeatures;
	use ldk_node::lightning::types::payment::PaymentHash;
	use ldk_node::lightning::util::ser::Writeable;
	use ldk_server_grpc::types::blinded_path::IntroductionNode;

	use super::*;

	fn pubkey(byte: u8) -> PublicKey {
		let secp = Secp256k1::new();
		PublicKey::from_secret_key(&secp, &SecretKey::from_slice(&[byte; 32]).unwrap())
	}

	/// The keypair the sample BOLT12 invoice is signed with; its public key is the
	/// invoice's `signing_pubkey`.
	fn signing_keypair() -> Keypair {
		let secp = Secp256k1::new();
		Keypair::from_secret_key(&secp, &SecretKey::from_slice(&[43; 32]).unwrap())
	}

	/// Builds a signed BOLT12 invoice and returns it hex-encoded, matching how a BOLT12
	/// invoice would be supplied to `DecodeInvoice`.
	fn sample_bolt12_invoice_hex() -> String {
		let secp = Secp256k1::new();
		let keys = signing_keypair();

		let payment_paths = vec![BlindedPaymentPath::from_blinded_path_and_payinfo(
			pubkey(40),
			pubkey(41),
			vec![
				BlindedHop { blinded_node_id: pubkey(43), encrypted_payload: vec![0; 43] },
				BlindedHop { blinded_node_id: pubkey(44), encrypted_payload: vec![0; 44] },
			],
			BlindedPayInfo {
				fee_base_msat: 1,
				fee_proportional_millionths: 1_000,
				cltv_expiry_delta: 42,
				htlc_minimum_msat: 100,
				htlc_maximum_msat: 1_000_000_000_000,
				features: BlindedHopFeatures::empty(),
			},
		)];

		let refund = RefundBuilder::new(vec![1; 32], pubkey(42), 1_000).unwrap().build().unwrap();
		let invoice = refund
			.respond_with(payment_paths, PaymentHash([42; 32]), keys.public_key())
			.unwrap()
			.relative_expiry(3600)
			.build()
			.unwrap()
			.sign(|message: &UnsignedBolt12Invoice| {
				Ok::<_, ()>(secp.sign_schnorr_no_aux_rand(message.as_ref().as_digest(), &keys))
			})
			.unwrap();

		let mut buffer = Vec::new();
		invoice.write(&mut buffer).unwrap();
		buffer.to_lower_hex_string()
	}

	#[test]
	fn rejects_unparseable_input() {
		assert!(decode_invoice("not an invoice").is_err());
	}

	#[test]
	fn rejects_hex_that_is_not_a_bolt12_invoice() {
		// Valid hex, but not a BOLT12 invoice TLV stream.
		assert!(decode_invoice("00010203").is_err());
	}

	#[test]
	fn decodes_bolt12_invoice_and_populates_fields() {
		let response = decode_invoice(&sample_bolt12_invoice_hex()).unwrap();
		assert_eq!(response.kind, INVOICE_KIND_BOLT12);
		assert_eq!(response.destination, signing_keypair().public_key().to_string());
		assert_eq!(response.payment_hash, "2a".repeat(32));
		assert_eq!(response.amount_msat, Some(1_000));
		assert_eq!(response.expiry, 3600);
		assert!(!response.is_expired);

		// The sample invoice carries a single blinded payment path with two hops,
		// introduced by `pubkey(40)` and blinded with `pubkey(41)`.
		assert_eq!(response.paths.len(), 1);
		let path = &response.paths[0];
		assert_eq!(path.num_hops, 2);
		assert_eq!(path.blinding_point, pubkey(41).to_string());
		assert_eq!(path.introduction_node, Some(IntroductionNode::NodeId(pubkey(40).to_string())));
	}
}
