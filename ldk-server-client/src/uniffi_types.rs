// UniFFI-exposed types and client wrapper for `ldk-server-client`.
//
// The types in this module are hand-written flat analogues of the
// prost-generated protobuf types from `ldk-server-grpc`. They exist because
// prost types are not directly UniFFI-exportable: they use
// `#[derive(::prost::Message)]`, nested `oneof` modules, and `prost::bytes::Bytes`,
// none of which UniFFI can serialize across the FFI boundary.
//
// Conversions (`From`/`Into`) to and from the underlying prost types are
// implemented alongside each wrapper.

use ldk_server_grpc::api::{
	Bolt11ReceiveResponse, Bolt12ReceiveResponse, DecodeInvoiceResponse, DecodeOfferResponse,
	GetBalancesResponse, GetNodeInfoResponse, UnifiedSendResponse,
};
use ldk_server_grpc::types::{
	offer_amount, payment_kind, BestBlock, Channel, OfferAmount, OutPoint, PageToken, Payment,
	PaymentDirection as ProstPaymentDirection, PaymentStatus as ProstPaymentStatus, Peer,
};

use crate::error::{LdkServerError, LdkServerErrorCode};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors surfaced across the UniFFI boundary by `LdkServerClientUni`.
///
/// Flat variants that map the server-side gRPC error codes plus a catch-all
/// for client-side / unknown errors.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum LdkServerClientError {
	#[error("Invalid request: {message}")]
	InvalidRequest { message: String },
	#[error("Authentication failed: {message}")]
	AuthenticationFailed { message: String },
	#[error("Lightning error: {message}")]
	LightningError { message: String },
	#[error("Internal server error: {message}")]
	InternalServerError { message: String },
	#[error("Internal error: {message}")]
	InternalError { message: String },
}

impl From<LdkServerError> for LdkServerClientError {
	fn from(err: LdkServerError) -> Self {
		let message = err.message;
		match err.error_code {
			LdkServerErrorCode::InvalidRequestError => Self::InvalidRequest { message },
			LdkServerErrorCode::AuthError => Self::AuthenticationFailed { message },
			LdkServerErrorCode::LightningError => Self::LightningError { message },
			LdkServerErrorCode::InternalServerError => Self::InternalServerError { message },
			LdkServerErrorCode::InternalError => Self::InternalError { message },
		}
	}
}

// ---------------------------------------------------------------------------
// Node info / best block
// ---------------------------------------------------------------------------

/// High-level identity and sync state of the remote LDK Server node.
#[derive(Clone, Debug, uniffi::Record)]
pub struct NodeInfo {
	/// Hex-encoded public key of the node.
	pub node_id: String,
	/// Best block the lightning wallet is currently synced to.
	pub current_best_block: Option<BestBlockInfo>,
	pub latest_lightning_wallet_sync_timestamp: Option<u64>,
	pub latest_onchain_wallet_sync_timestamp: Option<u64>,
	pub latest_fee_rate_cache_update_timestamp: Option<u64>,
	pub latest_rgs_snapshot_timestamp: Option<u64>,
	pub latest_node_announcement_broadcast_timestamp: Option<u64>,
	pub listening_addresses: Vec<String>,
	pub announcement_addresses: Vec<String>,
	pub node_alias: Option<String>,
	/// `node_id@address` strings that can be shared with peers.
	pub node_uris: Vec<String>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct BestBlockInfo {
	pub block_hash: String,
	pub height: u32,
}

impl From<BestBlock> for BestBlockInfo {
	fn from(b: BestBlock) -> Self {
		Self { block_hash: b.block_hash, height: b.height }
	}
}

impl From<GetNodeInfoResponse> for NodeInfo {
	fn from(r: GetNodeInfoResponse) -> Self {
		Self {
			node_id: r.node_id,
			current_best_block: r.current_best_block.map(BestBlockInfo::from),
			latest_lightning_wallet_sync_timestamp: r.latest_lightning_wallet_sync_timestamp,
			latest_onchain_wallet_sync_timestamp: r.latest_onchain_wallet_sync_timestamp,
			latest_fee_rate_cache_update_timestamp: r.latest_fee_rate_cache_update_timestamp,
			latest_rgs_snapshot_timestamp: r.latest_rgs_snapshot_timestamp,
			latest_node_announcement_broadcast_timestamp: r
				.latest_node_announcement_broadcast_timestamp,
			listening_addresses: r.listening_addresses,
			announcement_addresses: r.announcement_addresses,
			node_alias: r.node_alias,
			node_uris: r.node_uris,
		}
	}
}

// ---------------------------------------------------------------------------
// Balances
// ---------------------------------------------------------------------------

/// Summary of on-chain and lightning balances.
///
/// The detailed per-HTLC `lightning_balances` and `pending_balances_from_channel_closures`
/// breakdowns are intentionally omitted here: those variants involve deeply nested `oneof`
/// payloads that require substantially more wrapper scaffolding. The summary `*_sats` fields
/// are what a wallet UI displays; callers who need the breakdown can fall back to the raw
/// Rust client.
#[derive(Clone, Debug, uniffi::Record)]
pub struct BalanceInfo {
	pub total_onchain_balance_sats: u64,
	pub spendable_onchain_balance_sats: u64,
	pub total_anchor_channels_reserve_sats: u64,
	pub total_lightning_balance_sats: u64,
}

impl From<GetBalancesResponse> for BalanceInfo {
	fn from(r: GetBalancesResponse) -> Self {
		Self {
			total_onchain_balance_sats: r.total_onchain_balance_sats,
			spendable_onchain_balance_sats: r.spendable_onchain_balance_sats,
			total_anchor_channels_reserve_sats: r.total_anchor_channels_reserve_sats,
			total_lightning_balance_sats: r.total_lightning_balance_sats,
		}
	}
}

// ---------------------------------------------------------------------------
// Payments
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum PaymentDirection {
	Inbound,
	Outbound,
}

impl From<ProstPaymentDirection> for PaymentDirection {
	fn from(d: ProstPaymentDirection) -> Self {
		match d {
			ProstPaymentDirection::Inbound => Self::Inbound,
			ProstPaymentDirection::Outbound => Self::Outbound,
		}
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum PaymentStatus {
	Pending,
	Succeeded,
	Failed,
}

impl From<ProstPaymentStatus> for PaymentStatus {
	fn from(s: ProstPaymentStatus) -> Self {
		match s {
			ProstPaymentStatus::Pending => Self::Pending,
			ProstPaymentStatus::Succeeded => Self::Succeeded,
			ProstPaymentStatus::Failed => Self::Failed,
		}
	}
}

/// Flat representation of `PaymentKind`'s `oneof` variants.
///
/// Secret byte fields (`Bolt11::secret`, etc.) and the nested `ConfirmationStatus` on the
/// on-chain variant are dropped — a wallet UI only needs the outer identifiers (txid, hash,
/// preimage, offer_id).
#[derive(Clone, Debug, uniffi::Enum)]
pub enum PaymentKindInfo {
	Onchain { txid: String },
	Bolt11 { hash: String, preimage: Option<String> },
	Bolt11Jit { hash: String, preimage: Option<String> },
	Bolt12Offer { hash: Option<String>, preimage: Option<String>, offer_id: String },
	Bolt12Refund { hash: Option<String>, preimage: Option<String> },
	Spontaneous { hash: String, preimage: Option<String> },
}

impl From<payment_kind::Kind> for PaymentKindInfo {
	fn from(k: payment_kind::Kind) -> Self {
		match k {
			payment_kind::Kind::Onchain(o) => Self::Onchain { txid: o.txid },
			payment_kind::Kind::Bolt11(b) => Self::Bolt11 { hash: b.hash, preimage: b.preimage },
			payment_kind::Kind::Bolt11Jit(b) => {
				Self::Bolt11Jit { hash: b.hash, preimage: b.preimage }
			},
			payment_kind::Kind::Bolt12Offer(b) => {
				Self::Bolt12Offer { hash: b.hash, preimage: b.preimage, offer_id: b.offer_id }
			},
			payment_kind::Kind::Bolt12Refund(b) => {
				Self::Bolt12Refund { hash: b.hash, preimage: b.preimage }
			},
			payment_kind::Kind::Spontaneous(s) => {
				Self::Spontaneous { hash: s.hash, preimage: s.preimage }
			},
		}
	}
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct PaymentInfo {
	pub id: String,
	pub kind: Option<PaymentKindInfo>,
	pub amount_msat: Option<u64>,
	pub fee_paid_msat: Option<u64>,
	pub direction: PaymentDirection,
	pub status: PaymentStatus,
	pub latest_update_timestamp: u64,
}

impl From<Payment> for PaymentInfo {
	fn from(p: Payment) -> Self {
		// prost 0.11 generates `from_i32(i32) -> Option<Self>` for message enums, not
		// TryFrom. Unknown values (e.g., protocol version skew) fall back to safe defaults.
		let direction = ProstPaymentDirection::from_i32(p.direction)
			.unwrap_or(ProstPaymentDirection::Outbound)
			.into();
		let status =
			ProstPaymentStatus::from_i32(p.status).unwrap_or(ProstPaymentStatus::Pending).into();
		let kind = p.kind.and_then(|k| k.kind).map(PaymentKindInfo::from);
		Self {
			id: p.id,
			kind,
			amount_msat: p.amount_msat,
			fee_paid_msat: p.fee_paid_msat,
			direction,
			status,
			latest_update_timestamp: p.latest_update_timestamp,
		}
	}
}

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, uniffi::Record)]
pub struct OutPointInfo {
	pub txid: String,
	pub vout: u32,
}

impl From<OutPoint> for OutPointInfo {
	fn from(o: OutPoint) -> Self {
		Self { txid: o.txid, vout: o.vout }
	}
}

/// Subset of `Channel` fields relevant to a wallet UI. The fuller prost `Channel` has 25+
/// fields covering config, HTLC limits, and counterparty forwarding info — all omitted here.
#[derive(Clone, Debug, uniffi::Record)]
pub struct ChannelInfo {
	pub channel_id: String,
	pub counterparty_node_id: String,
	pub funding_txo: Option<OutPointInfo>,
	pub user_channel_id: String,
	pub channel_value_sats: u64,
	pub outbound_capacity_msat: u64,
	pub inbound_capacity_msat: u64,
	pub confirmations_required: Option<u32>,
	pub confirmations: Option<u32>,
	pub is_outbound: bool,
	pub is_channel_ready: bool,
	pub is_usable: bool,
	pub is_announced: bool,
}

impl From<Channel> for ChannelInfo {
	fn from(c: Channel) -> Self {
		Self {
			channel_id: c.channel_id,
			counterparty_node_id: c.counterparty_node_id,
			funding_txo: c.funding_txo.map(OutPointInfo::from),
			user_channel_id: c.user_channel_id,
			channel_value_sats: c.channel_value_sats,
			outbound_capacity_msat: c.outbound_capacity_msat,
			inbound_capacity_msat: c.inbound_capacity_msat,
			confirmations_required: c.confirmations_required,
			confirmations: c.confirmations,
			is_outbound: c.is_outbound,
			is_channel_ready: c.is_channel_ready,
			is_usable: c.is_usable,
			is_announced: c.is_announced,
		}
	}
}

// ---------------------------------------------------------------------------
// Peers
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, uniffi::Record)]
pub struct PeerInfo {
	pub node_id: String,
	pub address: String,
	pub is_persisted: bool,
	pub is_connected: bool,
}

impl From<Peer> for PeerInfo {
	fn from(p: Peer) -> Self {
		Self {
			node_id: p.node_id,
			address: p.address,
			is_persisted: p.is_persisted,
			is_connected: p.is_connected,
		}
	}
}

// ---------------------------------------------------------------------------
// Send / receive results
// ---------------------------------------------------------------------------

/// Result of `unified_send`. Which variant is produced depends on which of the candidate
/// payment types (offer → invoice → on-chain) the server found first in the supplied URI.
#[derive(Clone, Debug, uniffi::Enum)]
pub enum UnifiedSendResult {
	Onchain { txid: String },
	Bolt11 { payment_id: String },
	Bolt12 { payment_id: String },
}

impl TryFrom<UnifiedSendResponse> for UnifiedSendResult {
	type Error = LdkServerClientError;

	fn try_from(r: UnifiedSendResponse) -> Result<Self, Self::Error> {
		use ldk_server_grpc::api::unified_send_response::PaymentResult;
		match r.payment_result {
			Some(PaymentResult::Txid(txid)) => Ok(Self::Onchain { txid }),
			Some(PaymentResult::Bolt11PaymentId(payment_id)) => Ok(Self::Bolt11 { payment_id }),
			Some(PaymentResult::Bolt12PaymentId(payment_id)) => Ok(Self::Bolt12 { payment_id }),
			None => Err(LdkServerClientError::InternalError {
				message: "server returned UnifiedSendResponse with no payment_result".to_string(),
			}),
		}
	}
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct Bolt11ReceiveResult {
	pub invoice: String,
	pub payment_hash: String,
	pub payment_secret: String,
}

impl From<Bolt11ReceiveResponse> for Bolt11ReceiveResult {
	fn from(r: Bolt11ReceiveResponse) -> Self {
		Self { invoice: r.invoice, payment_hash: r.payment_hash, payment_secret: r.payment_secret }
	}
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct Bolt12ReceiveResult {
	pub offer: String,
	pub offer_id: String,
}

impl From<Bolt12ReceiveResponse> for Bolt12ReceiveResult {
	fn from(r: Bolt12ReceiveResponse) -> Self {
		Self { offer: r.offer, offer_id: r.offer_id }
	}
}

// ---------------------------------------------------------------------------
// Decoded invoice / offer
// ---------------------------------------------------------------------------

/// Parsed BOLT11 invoice.
///
/// `route_hints` and the `features` bitmap are dropped — they aren't useful to a mobile
/// wallet UI and require more wrapper types.
#[derive(Clone, Debug, uniffi::Record)]
pub struct DecodedInvoice {
	pub destination: String,
	pub payment_hash: String,
	pub amount_msat: Option<u64>,
	pub timestamp: u64,
	pub expiry: u64,
	pub description: Option<String>,
	pub description_hash: Option<String>,
	pub fallback_address: Option<String>,
	pub min_final_cltv_expiry_delta: u64,
	pub payment_secret: String,
	pub currency: String,
	pub payment_metadata: Option<String>,
	pub is_expired: bool,
}

impl From<DecodeInvoiceResponse> for DecodedInvoice {
	fn from(r: DecodeInvoiceResponse) -> Self {
		Self {
			destination: r.destination,
			payment_hash: r.payment_hash,
			amount_msat: r.amount_msat,
			timestamp: r.timestamp,
			expiry: r.expiry,
			description: r.description,
			description_hash: r.description_hash,
			fallback_address: r.fallback_address,
			min_final_cltv_expiry_delta: r.min_final_cltv_expiry_delta,
			payment_secret: r.payment_secret,
			currency: r.currency,
			payment_metadata: r.payment_metadata,
			is_expired: r.is_expired,
		}
	}
}

/// Parsed BOLT12 offer.
///
/// The prost `OfferAmount` is a `oneof` with a `BitcoinAmountMsats(u64)` and a
/// `CurrencyAmount{iso4217_code, amount}` variant. Wallets overwhelmingly only care about
/// the Bitcoin variant, so we flatten to `amount_msat: Option<u64>` and drop currency
/// offers. `features`, `paths`, `metadata`, and `quantity` are also dropped for MVP.
#[derive(Clone, Debug, uniffi::Record)]
pub struct DecodedOffer {
	pub offer_id: String,
	pub description: Option<String>,
	pub issuer: Option<String>,
	pub amount_msat: Option<u64>,
	pub issuer_signing_pubkey: Option<String>,
	pub absolute_expiry: Option<u64>,
	pub chains: Vec<String>,
	pub is_expired: bool,
}

fn extract_bitcoin_amount_msats(amount: Option<OfferAmount>) -> Option<u64> {
	match amount?.amount? {
		offer_amount::Amount::BitcoinAmountMsats(msats) => Some(msats),
		offer_amount::Amount::CurrencyAmount(_) => None,
	}
}

impl From<DecodeOfferResponse> for DecodedOffer {
	fn from(r: DecodeOfferResponse) -> Self {
		Self {
			offer_id: r.offer_id,
			description: r.description,
			issuer: r.issuer,
			amount_msat: extract_bitcoin_amount_msats(r.amount),
			issuer_signing_pubkey: r.issuer_signing_pubkey,
			absolute_expiry: r.absolute_expiry,
			chains: r.chains,
			is_expired: r.is_expired,
		}
	}
}

// ---------------------------------------------------------------------------
// Pagination / list results
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, uniffi::Record)]
pub struct PageTokenInfo {
	pub token: String,
	pub index: i64,
}

impl From<PageToken> for PageTokenInfo {
	fn from(t: PageToken) -> Self {
		Self { token: t.token, index: t.index }
	}
}

impl From<PageTokenInfo> for PageToken {
	fn from(t: PageTokenInfo) -> Self {
		Self { token: t.token, index: t.index }
	}
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ListPaymentsResult {
	pub payments: Vec<PaymentInfo>,
	pub next_page_token: Option<PageTokenInfo>,
}
