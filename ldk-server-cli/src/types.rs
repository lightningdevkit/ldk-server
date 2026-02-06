// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! CLI-specific type wrappers for API responses.
//!
//! This file contains wrapper types that customize the serialization format
//! of API responses for CLI output. These wrappers ensure that the CLI's output
//! format matches what users expect and what the CLI can parse back as input.

use hex_conservative::DisplayHex;
use ldk_server_client::ldk_server_protos::types::{
	payment_kind, Bolt11, Bolt11Jit, Bolt12Offer, Bolt12Refund, ForwardedPayment, Onchain,
	PageToken, Payment, PaymentKind, Spontaneous,
};
use serde::Serialize;

/// CLI-specific wrapper for paginated responses that formats the page token
/// as "token:idx" instead of a JSON object.
#[derive(Debug, Clone, Serialize)]
pub struct CliPaginatedResponse<T> {
	/// List of items.
	pub list: Vec<T>,
	/// Next page token formatted as "token:idx", or None if no more pages.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub next_page_token: Option<String>,
}

impl<T> CliPaginatedResponse<T> {
	pub fn new(list: Vec<T>, next_page_token: Option<PageToken>) -> Self {
		Self { list, next_page_token: next_page_token.map(format_page_token) }
	}
}

pub type CliListPaymentsResponse = CliPaginatedResponse<CliPayment>;
pub type CliListForwardedPaymentsResponse = CliPaginatedResponse<ForwardedPayment>;

fn format_page_token(token: PageToken) -> String {
	format!("{}:{}", token.token, token.index)
}

impl From<ldk_server_client::ldk_server_protos::api::ListPaymentsResponse>
	for CliListPaymentsResponse
{
	fn from(response: ldk_server_client::ldk_server_protos::api::ListPaymentsResponse) -> Self {
		CliPaginatedResponse::new(
			response.payments.into_iter().map(Into::into).collect(),
			response.next_page_token,
		)
	}
}

impl From<CliPaginatedResponse<Payment>> for CliPaginatedResponse<CliPayment> {
	fn from(response: CliPaginatedResponse<Payment>) -> Self {
		CliPaginatedResponse {
			list: response.list.into_iter().map(Into::into).collect(),
			next_page_token: response.next_page_token,
		}
	}
}

/// CLI-specific wrapper for GetPaymentDetailsResponse.
#[derive(Debug, Clone, Serialize)]
pub struct CliGetPaymentDetailsResponse {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub payment: Option<CliPayment>,
}

impl From<ldk_server_client::ldk_server_protos::api::GetPaymentDetailsResponse>
	for CliGetPaymentDetailsResponse
{
	fn from(
		response: ldk_server_client::ldk_server_protos::api::GetPaymentDetailsResponse,
	) -> Self {
		Self { payment: response.payment.map(Into::into) }
	}
}

/// CLI-specific wrapper for bytes that serializes to hex string.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct CliBytes(String);

impl<T: AsRef<[u8]>> From<T> for CliBytes {
	fn from(bytes: T) -> Self {
		Self(format!("{}", bytes.as_ref().as_hex()))
	}
}

/// CLI-specific wrapper for BOLT 11 payment.
#[derive(Debug, Clone, Serialize)]
pub struct CliBolt11 {
	pub hash: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub preimage: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub secret: Option<CliBytes>,
}

impl From<Bolt11> for CliBolt11 {
	fn from(bolt11: Bolt11) -> Self {
		Self { hash: bolt11.hash, preimage: bolt11.preimage, secret: bolt11.secret.map(Into::into) }
	}
}

/// CLI-specific wrapper for BOLT 11 JIT payment.
#[derive(Debug, Clone, Serialize)]
pub struct CliBolt11Jit {
	pub hash: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub preimage: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub secret: Option<CliBytes>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub lsp_fee_limits: Option<ldk_server_client::ldk_server_protos::types::LspFeeLimits>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub counterparty_skimmed_fee_msat: Option<u64>,
}

impl From<Bolt11Jit> for CliBolt11Jit {
	fn from(jit: Bolt11Jit) -> Self {
		Self {
			hash: jit.hash,
			preimage: jit.preimage,
			secret: jit.secret.map(Into::into),
			lsp_fee_limits: jit.lsp_fee_limits,
			counterparty_skimmed_fee_msat: jit.counterparty_skimmed_fee_msat,
		}
	}
}

/// CLI-specific wrapper for BOLT 12 Offer payment.
#[derive(Debug, Clone, Serialize)]
pub struct CliBolt12Offer {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub hash: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub preimage: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub secret: Option<CliBytes>,
	pub offer_id: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub payer_note: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub quantity: Option<u64>,
}

impl From<Bolt12Offer> for CliBolt12Offer {
	fn from(offer: Bolt12Offer) -> Self {
		Self {
			hash: offer.hash,
			preimage: offer.preimage,
			secret: offer.secret.map(Into::into),
			offer_id: offer.offer_id,
			payer_note: offer.payer_note,
			quantity: offer.quantity,
		}
	}
}

/// CLI-specific wrapper for BOLT 12 Refund payment.
#[derive(Debug, Clone, Serialize)]
pub struct CliBolt12Refund {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub hash: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub preimage: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub secret: Option<CliBytes>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub payer_note: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub quantity: Option<u64>,
}

impl From<Bolt12Refund> for CliBolt12Refund {
	fn from(refund: Bolt12Refund) -> Self {
		Self {
			hash: refund.hash,
			preimage: refund.preimage,
			secret: refund.secret.map(Into::into),
			payer_note: refund.payer_note,
			quantity: refund.quantity,
		}
	}
}

/// CLI-specific wrapper for Spontaneous payment.
#[derive(Debug, Clone, Serialize)]
pub struct CliSpontaneous {
	pub hash: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub preimage: Option<String>,
}

impl From<Spontaneous> for CliSpontaneous {
	fn from(spontaneous: Spontaneous) -> Self {
		Self { hash: spontaneous.hash, preimage: spontaneous.preimage }
	}
}

/// CLI-specific wrapper for PaymentKind.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliPaymentKind {
	Onchain(Onchain),
	Bolt11(CliBolt11),
	Bolt11Jit(CliBolt11Jit),
	Bolt12Offer(CliBolt12Offer),
	Bolt12Refund(CliBolt12Refund),
	Spontaneous(CliSpontaneous),
}

impl CliPaymentKind {
	pub fn from_payment_kind(kind: Option<PaymentKind>) -> Option<Self> {
		kind.and_then(|k| {
			k.kind.map(|inner| match inner {
				payment_kind::Kind::Onchain(o) => CliPaymentKind::Onchain(o),
				payment_kind::Kind::Bolt11(b) => CliPaymentKind::Bolt11(b.into()),
				payment_kind::Kind::Bolt11Jit(j) => CliPaymentKind::Bolt11Jit(j.into()),
				payment_kind::Kind::Bolt12Offer(o) => CliPaymentKind::Bolt12Offer(o.into()),
				payment_kind::Kind::Bolt12Refund(r) => CliPaymentKind::Bolt12Refund(r.into()),
				payment_kind::Kind::Spontaneous(s) => CliPaymentKind::Spontaneous(s.into()),
			})
		})
	}
}

/// CLI-specific wrapper for Payment that formats enums and bytes for readability.
#[derive(Debug, Clone, Serialize)]
pub struct CliPayment {
	pub id: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub kind: Option<CliPaymentKind>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub amount_msat: Option<u64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub fee_paid_msat: Option<u64>,
	pub direction: String,
	pub status: String,
	pub latest_update_timestamp: u64,
}

impl From<Payment> for CliPayment {
	fn from(payment: Payment) -> Self {
		let direction = payment.direction().as_str_name().to_string();
		let status = payment.status().as_str_name().to_string();

		Self {
			id: payment.id,
			kind: CliPaymentKind::from_payment_kind(payment.kind),
			amount_msat: payment.amount_msat,
			fee_paid_msat: payment.fee_paid_msat,
			direction,
			status,
			latest_update_timestamp: payment.latest_update_timestamp,
		}
	}
}
