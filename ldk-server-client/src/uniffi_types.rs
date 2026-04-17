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

use std::sync::Arc;

use ldk_server_grpc::api::{
	Bolt11ReceiveRequest, Bolt11ReceiveResponse, Bolt11SendRequest, Bolt12ReceiveRequest,
	Bolt12ReceiveResponse, Bolt12SendRequest, CloseChannelRequest, ConnectPeerRequest,
	DecodeInvoiceRequest, DecodeInvoiceResponse, DecodeOfferRequest, DecodeOfferResponse,
	DisconnectPeerRequest, ForceCloseChannelRequest, GetBalancesRequest, GetBalancesResponse,
	GetNodeInfoRequest, GetNodeInfoResponse, GetPaymentDetailsRequest, ListChannelsRequest,
	ListPaymentsRequest, ListPeersRequest, OnchainReceiveRequest, OnchainSendRequest,
	OpenChannelRequest, UnifiedSendRequest, UnifiedSendResponse,
};
use ldk_server_grpc::types::{
	bolt11_invoice_description, offer_amount, payment_kind, BestBlock, Bolt11InvoiceDescription,
	Channel, OfferAmount, OutPoint, PageToken, Payment, PaymentDirection as ProstPaymentDirection,
	PaymentStatus as ProstPaymentStatus, Peer,
};

use crate::client::LdkServerClient;
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
	#[error("Invalid request: {reason}")]
	InvalidRequest { reason: String },
	#[error("Authentication failed: {reason}")]
	AuthenticationFailed { reason: String },
	#[error("Lightning error: {reason}")]
	LightningError { reason: String },
	#[error("Internal server error: {reason}")]
	InternalServerError { reason: String },
	#[error("Internal error: {reason}")]
	InternalError { reason: String },
}

impl From<LdkServerError> for LdkServerClientError {
	fn from(err: LdkServerError) -> Self {
		// NOTE: the field is deliberately named `reason` rather than `message`: UniFFI's
		// Kotlin generator emits struct-variant errors as subclasses of `Throwable`, and a
		// constructor property called `message` collides with Throwable's own `message`
		// property, producing a compile error on the generated code. `reason` sidesteps that.
		let reason = err.message;
		match err.error_code {
			LdkServerErrorCode::InvalidRequestError => Self::InvalidRequest { reason },
			LdkServerErrorCode::AuthError => Self::AuthenticationFailed { reason },
			LdkServerErrorCode::LightningError => Self::LightningError { reason },
			LdkServerErrorCode::InternalServerError => Self::InternalServerError { reason },
			LdkServerErrorCode::InternalError => Self::InternalError { reason },
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
				reason: "server returned UnifiedSendResponse with no payment_result".to_string(),
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

// ---------------------------------------------------------------------------
// Client wrapper
// ---------------------------------------------------------------------------

/// UniFFI-exported wrapper around [`LdkServerClient`].
///
/// `LdkServerClient` itself holds `reqwest::Client` + `hyper::Client`, which are not
/// UniFFI-exportable. This thin wrapper adapts the async API to `Arc<Self>` / `suspend fun`
/// semantics that UniFFI generates for Kotlin (and Swift).
#[derive(uniffi::Object)]
pub struct LdkServerClientUni {
	inner: LdkServerClient,
}

#[uniffi::export(async_runtime = "tokio")]
impl LdkServerClientUni {
	/// Construct a new client.
	///
	/// - `base_url` should omit the scheme (e.g. `"localhost:3000"`).
	/// - `api_key` is the hex-encoded 32-byte key that the server generates on first run;
	///   it's used for HMAC-SHA256 request authentication.
	/// - `server_cert_pem` is the text of the server's TLS certificate; found at
	///   `<server_storage_dir>/tls.crt` after the server has started.
	#[uniffi::constructor]
	pub fn new(
		base_url: String, api_key: String, server_cert_pem: String,
	) -> Result<Arc<Self>, LdkServerClientError> {
		let inner = LdkServerClient::new(base_url, api_key, server_cert_pem.as_bytes())
			.map_err(|reason| LdkServerClientError::InvalidRequest { reason })?;
		Ok(Arc::new(Self { inner }))
	}

	/// Retrieve the node's identity, sync status, and announced addresses.
	pub async fn get_node_info(&self) -> Result<NodeInfo, LdkServerClientError> {
		let resp = self.inner.get_node_info(GetNodeInfoRequest {}).await?;
		Ok(resp.into())
	}

	/// Retrieve the summary on-chain and Lightning balances.
	pub async fn get_balances(&self) -> Result<BalanceInfo, LdkServerClientError> {
		let resp = self.inner.get_balances(GetBalancesRequest {}).await?;
		Ok(resp.into())
	}

	/// List all known channels.
	pub async fn list_channels(&self) -> Result<Vec<ChannelInfo>, LdkServerClientError> {
		let resp = self.inner.list_channels(ListChannelsRequest {}).await?;
		Ok(resp.channels.into_iter().map(ChannelInfo::from).collect())
	}

	/// List all known peers (connected or persisted).
	pub async fn list_peers(&self) -> Result<Vec<PeerInfo>, LdkServerClientError> {
		let resp = self.inner.list_peers(ListPeersRequest {}).await?;
		Ok(resp.peers.into_iter().map(PeerInfo::from).collect())
	}

	/// List payments. Pass the `next_page_token` from a prior result to continue pagination.
	pub async fn list_payments(
		&self, page_token: Option<PageTokenInfo>,
	) -> Result<ListPaymentsResult, LdkServerClientError> {
		let request = ListPaymentsRequest { page_token: page_token.map(PageToken::from) };
		let resp = self.inner.list_payments(request).await?;
		Ok(ListPaymentsResult {
			payments: resp.payments.into_iter().map(PaymentInfo::from).collect(),
			next_page_token: resp.next_page_token.map(PageTokenInfo::from),
		})
	}

	/// Fetch the details for a single payment by id. Returns `None` if no payment with that
	/// id is known.
	pub async fn get_payment_details(
		&self, payment_id: String,
	) -> Result<Option<PaymentInfo>, LdkServerClientError> {
		let resp = self.inner.get_payment_details(GetPaymentDetailsRequest { payment_id }).await?;
		Ok(resp.payment.map(PaymentInfo::from))
	}

	// ---- Receive -------------------------------------------------------

	/// Generate a new on-chain address to receive into. Each call yields a fresh address.
	pub async fn onchain_receive(&self) -> Result<String, LdkServerClientError> {
		let resp = self.inner.onchain_receive(OnchainReceiveRequest {}).await?;
		Ok(resp.address)
	}

	/// Generate a new BOLT11 invoice. `description`, if supplied, is attached as a direct
	/// description (the hash variant is intentionally not exposed for the MVP).
	pub async fn bolt11_receive(
		&self, amount_msat: Option<u64>, description: Option<String>, expiry_secs: u32,
	) -> Result<Bolt11ReceiveResult, LdkServerClientError> {
		let description = description.map(|s| Bolt11InvoiceDescription {
			kind: Some(bolt11_invoice_description::Kind::Direct(s)),
		});
		let request = Bolt11ReceiveRequest { amount_msat, description, expiry_secs };
		let resp = self.inner.bolt11_receive(request).await?;
		Ok(resp.into())
	}

	/// Generate a new BOLT12 offer. `description` is required (pass an empty string if you
	/// want no description).
	pub async fn bolt12_receive(
		&self, description: String, amount_msat: Option<u64>, expiry_secs: Option<u32>,
		quantity: Option<u64>,
	) -> Result<Bolt12ReceiveResult, LdkServerClientError> {
		let request = Bolt12ReceiveRequest { description, amount_msat, expiry_secs, quantity };
		let resp = self.inner.bolt12_receive(request).await?;
		Ok(resp.into())
	}

	// ---- Send ----------------------------------------------------------

	/// Pay a BIP21 URI or BIP353 Human-Readable Name. Dispatches to on-chain, BOLT11, or
	/// BOLT12 on the server side depending on what the URI resolves to.
	pub async fn unified_send(
		&self, uri: String, amount_msat: Option<u64>,
	) -> Result<UnifiedSendResult, LdkServerClientError> {
		let request = UnifiedSendRequest { uri, amount_msat, route_parameters: None };
		let resp = self.inner.unified_send(request).await?;
		UnifiedSendResult::try_from(resp)
	}

	/// Pay a BOLT11 invoice. `amount_msat` is required for zero-amount invoices and must be
	/// `None` for fixed-amount invoices. Returns the server-side payment id.
	pub async fn bolt11_send(
		&self, invoice: String, amount_msat: Option<u64>,
	) -> Result<String, LdkServerClientError> {
		let request = Bolt11SendRequest { invoice, amount_msat, route_parameters: None };
		let resp = self.inner.bolt11_send(request).await?;
		Ok(resp.payment_id)
	}

	/// Pay a BOLT12 offer. Returns the server-side payment id.
	pub async fn bolt12_send(
		&self, offer: String, amount_msat: Option<u64>, quantity: Option<u64>,
		payer_note: Option<String>,
	) -> Result<String, LdkServerClientError> {
		let request =
			Bolt12SendRequest { offer, amount_msat, quantity, payer_note, route_parameters: None };
		let resp = self.inner.bolt12_send(request).await?;
		Ok(resp.payment_id)
	}

	/// Send an on-chain payment. Set `send_all = Some(true)` to sweep the wallet (dangerous
	/// if anchor channels are open — see `OnchainSendRequest` docs in the proto file).
	/// Returns the broadcast txid.
	pub async fn onchain_send(
		&self, address: String, amount_sats: Option<u64>, send_all: Option<bool>,
		fee_rate_sat_per_vb: Option<u64>,
	) -> Result<String, LdkServerClientError> {
		let request = OnchainSendRequest { address, amount_sats, send_all, fee_rate_sat_per_vb };
		let resp = self.inner.onchain_send(request).await?;
		Ok(resp.txid)
	}

	// ---- Channels ------------------------------------------------------

	/// Open a new channel with the given peer. Returns the local `user_channel_id`.
	pub async fn open_channel(
		&self, node_pubkey: String, address: String, channel_amount_sats: u64,
		push_to_counterparty_msat: Option<u64>, announce_channel: bool,
	) -> Result<String, LdkServerClientError> {
		let request = OpenChannelRequest {
			node_pubkey,
			address,
			channel_amount_sats,
			push_to_counterparty_msat,
			channel_config: None,
			announce_channel,
			disable_counterparty_reserve: false,
		};
		let resp = self.inner.open_channel(request).await?;
		Ok(resp.user_channel_id)
	}

	/// Cooperatively close a channel.
	pub async fn close_channel(
		&self, user_channel_id: String, counterparty_node_id: String,
	) -> Result<(), LdkServerClientError> {
		let request = CloseChannelRequest { user_channel_id, counterparty_node_id };
		self.inner.close_channel(request).await?;
		Ok(())
	}

	/// Force-close a channel.
	pub async fn force_close_channel(
		&self, user_channel_id: String, counterparty_node_id: String,
		force_close_reason: Option<String>,
	) -> Result<(), LdkServerClientError> {
		let request =
			ForceCloseChannelRequest { user_channel_id, counterparty_node_id, force_close_reason };
		self.inner.force_close_channel(request).await?;
		Ok(())
	}

	// ---- Peers ---------------------------------------------------------

	/// Connect to a peer. If `persist = true`, we'll try to reconnect after restarts.
	pub async fn connect_peer(
		&self, node_pubkey: String, address: String, persist: bool,
	) -> Result<(), LdkServerClientError> {
		let request = ConnectPeerRequest { node_pubkey, address, persist };
		self.inner.connect_peer(request).await?;
		Ok(())
	}

	/// Disconnect from a peer.
	pub async fn disconnect_peer(&self, node_pubkey: String) -> Result<(), LdkServerClientError> {
		self.inner.disconnect_peer(DisconnectPeerRequest { node_pubkey }).await?;
		Ok(())
	}

	// ---- Decode --------------------------------------------------------

	/// Parse a BOLT11 invoice without sending a payment.
	pub async fn decode_invoice(
		&self, invoice: String,
	) -> Result<DecodedInvoice, LdkServerClientError> {
		let resp = self.inner.decode_invoice(DecodeInvoiceRequest { invoice }).await?;
		Ok(resp.into())
	}

	/// Parse a BOLT12 offer without sending a payment.
	pub async fn decode_offer(&self, offer: String) -> Result<DecodedOffer, LdkServerClientError> {
		let resp = self.inner.decode_offer(DecodeOfferRequest { offer }).await?;
		Ok(resp.into())
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use ldk_server_grpc::api::unified_send_response::PaymentResult;
	use ldk_server_grpc::api::Bolt11ReceiveResponse;
	use ldk_server_grpc::types::offer_amount::Amount as OfferAmountOneof;
	use ldk_server_grpc::types::payment_kind::Kind as PaymentKindProst;
	use ldk_server_grpc::types::{
		Bolt11, Bolt11Jit, Bolt12Offer, Bolt12Refund, CurrencyAmount, Onchain, PaymentKind,
		Spontaneous,
	};

	use super::*;

	#[test]
	fn error_code_mapping_covers_all_variants() {
		// Each prost error code maps to a distinct wrapper variant, and the Display output
		// preserves the original server-side message. We match on the wrapper variant
		// directly rather than on the Display prefix so this test doesn't break if we
		// retune wording.
		let cases: Vec<(LdkServerErrorCode, &str, fn(&LdkServerClientError) -> bool)> = vec![
			(LdkServerErrorCode::InvalidRequestError, "r1", |e| {
				matches!(e, LdkServerClientError::InvalidRequest { .. })
			}),
			(LdkServerErrorCode::AuthError, "r2", |e| {
				matches!(e, LdkServerClientError::AuthenticationFailed { .. })
			}),
			(LdkServerErrorCode::LightningError, "r3", |e| {
				matches!(e, LdkServerClientError::LightningError { .. })
			}),
			(LdkServerErrorCode::InternalServerError, "r4", |e| {
				matches!(e, LdkServerClientError::InternalServerError { .. })
			}),
			(LdkServerErrorCode::InternalError, "r5", |e| {
				matches!(e, LdkServerClientError::InternalError { .. })
			}),
		];
		for (code, msg, is_expected_variant) in cases {
			let err: LdkServerClientError = LdkServerError::new(code.clone(), msg).into();
			assert!(is_expected_variant(&err), "wrong variant for code {code:?}: {err:?}");
			assert!(
				format!("{err}").contains(msg),
				"display output should contain the original message ({msg})",
			);
		}
	}

	#[test]
	fn best_block_and_node_info_roundtrip() {
		let resp = GetNodeInfoResponse {
			node_id: "0211".to_string(),
			current_best_block: Some(BestBlock { block_hash: "abcd".to_string(), height: 42 }),
			latest_lightning_wallet_sync_timestamp: Some(100),
			latest_onchain_wallet_sync_timestamp: None,
			latest_fee_rate_cache_update_timestamp: Some(200),
			latest_rgs_snapshot_timestamp: None,
			latest_node_announcement_broadcast_timestamp: Some(300),
			listening_addresses: vec!["127.0.0.1:9735".to_string()],
			announcement_addresses: vec![],
			node_alias: Some("my-node".to_string()),
			node_uris: vec!["0211@example.com:9735".to_string()],
		};
		let info: NodeInfo = resp.into();
		assert_eq!(info.node_id, "0211");
		let best = info.current_best_block.expect("present");
		assert_eq!(best.height, 42);
		assert_eq!(best.block_hash, "abcd");
		assert_eq!(info.latest_lightning_wallet_sync_timestamp, Some(100));
		assert_eq!(info.latest_onchain_wallet_sync_timestamp, None);
		assert_eq!(info.node_alias.as_deref(), Some("my-node"));
	}

	#[test]
	fn node_info_handles_missing_best_block() {
		let info: NodeInfo = GetNodeInfoResponse::default().into();
		assert!(info.current_best_block.is_none());
		assert!(info.listening_addresses.is_empty());
		assert!(info.node_alias.is_none());
	}

	#[test]
	fn balance_info_from_defaults() {
		let balances: BalanceInfo = GetBalancesResponse::default().into();
		assert_eq!(balances.total_onchain_balance_sats, 0);
		assert_eq!(balances.total_lightning_balance_sats, 0);
	}

	#[test]
	fn payment_direction_and_status_mapping() {
		assert_eq!(
			PaymentDirection::from(ProstPaymentDirection::Inbound),
			PaymentDirection::Inbound
		);
		assert_eq!(
			PaymentDirection::from(ProstPaymentDirection::Outbound),
			PaymentDirection::Outbound
		);
		assert_eq!(PaymentStatus::from(ProstPaymentStatus::Pending), PaymentStatus::Pending);
		assert_eq!(PaymentStatus::from(ProstPaymentStatus::Succeeded), PaymentStatus::Succeeded);
		assert_eq!(PaymentStatus::from(ProstPaymentStatus::Failed), PaymentStatus::Failed);
	}

	#[test]
	fn payment_kind_covers_all_oneof_variants() {
		let cases: Vec<(PaymentKindProst, &str)> = vec![
			(
				PaymentKindProst::Onchain(Onchain { txid: "deadbeef".to_string(), status: None }),
				"Onchain",
			),
			(
				PaymentKindProst::Bolt11(Bolt11 {
					hash: "h11".to_string(),
					preimage: Some("p11".to_string()),
					secret: None,
				}),
				"Bolt11",
			),
			(
				PaymentKindProst::Bolt11Jit(Bolt11Jit {
					hash: "hjit".to_string(),
					preimage: None,
					secret: None,
					lsp_fee_limits: None,
					counterparty_skimmed_fee_msat: None,
				}),
				"Bolt11Jit",
			),
			(
				PaymentKindProst::Bolt12Offer(Bolt12Offer {
					hash: None,
					preimage: None,
					secret: None,
					offer_id: "oid".to_string(),
					payer_note: None,
					quantity: None,
				}),
				"Bolt12Offer",
			),
			(
				PaymentKindProst::Bolt12Refund(Bolt12Refund {
					hash: None,
					preimage: None,
					secret: None,
					payer_note: None,
					quantity: None,
				}),
				"Bolt12Refund",
			),
			(
				PaymentKindProst::Spontaneous(Spontaneous {
					hash: "hsp".to_string(),
					preimage: None,
				}),
				"Spontaneous",
			),
		];

		for (prost_kind, label) in cases {
			let wrapper = PaymentKindInfo::from(prost_kind);
			let debug_repr = format!("{wrapper:?}");
			assert!(debug_repr.starts_with(label), "{debug_repr} should start with {label}");
		}
	}

	#[test]
	fn payment_with_unknown_enum_values_defaults_safely() {
		// Protocol version skew: the server returns a direction/status int the client
		// doesn't recognize. We should fall back to safe defaults rather than panic.
		let payment = Payment {
			id: "x".to_string(),
			kind: Some(PaymentKind {
				kind: Some(PaymentKindProst::Spontaneous(Spontaneous {
					hash: "h".to_string(),
					preimage: None,
				})),
			}),
			amount_msat: Some(1000),
			fee_paid_msat: None,
			direction: 999,
			status: -1,
			latest_update_timestamp: 0,
		};
		let info: PaymentInfo = payment.into();
		assert_eq!(info.direction, PaymentDirection::Outbound);
		assert_eq!(info.status, PaymentStatus::Pending);
		assert!(info.kind.is_some());
	}

	#[test]
	fn channel_info_roundtrip() {
		let c = Channel {
			channel_id: "cid".to_string(),
			counterparty_node_id: "cp".to_string(),
			funding_txo: Some(OutPoint { txid: "tx".to_string(), vout: 3 }),
			user_channel_id: "uc".to_string(),
			unspendable_punishment_reserve: None,
			channel_value_sats: 1_000_000,
			feerate_sat_per_1000_weight: 0,
			outbound_capacity_msat: 500_000_000,
			inbound_capacity_msat: 500_000_000,
			confirmations_required: Some(6),
			confirmations: Some(3),
			is_outbound: true,
			is_channel_ready: false,
			is_usable: false,
			is_announced: true,
			channel_config: None,
			next_outbound_htlc_limit_msat: 0,
			next_outbound_htlc_minimum_msat: 0,
			force_close_spend_delay: None,
			counterparty_outbound_htlc_minimum_msat: None,
			counterparty_outbound_htlc_maximum_msat: None,
			counterparty_unspendable_punishment_reserve: 0,
			counterparty_forwarding_info_fee_base_msat: None,
			counterparty_forwarding_info_fee_proportional_millionths: None,
			counterparty_forwarding_info_cltv_expiry_delta: None,
		};
		let info: ChannelInfo = c.into();
		assert_eq!(info.channel_id, "cid");
		assert_eq!(info.funding_txo.as_ref().map(|o| o.vout), Some(3));
		assert_eq!(info.channel_value_sats, 1_000_000);
		assert!(info.is_outbound);
		assert!(!info.is_usable);
	}

	#[test]
	fn peer_info_roundtrip() {
		let peer = Peer {
			node_id: "np".to_string(),
			address: "127.0.0.1:9735".to_string(),
			is_persisted: true,
			is_connected: false,
		};
		let info: PeerInfo = peer.into();
		assert_eq!(info.node_id, "np");
		assert!(info.is_persisted);
		assert!(!info.is_connected);
	}

	#[test]
	fn unified_send_result_dispatches_on_oneof() {
		let txid_resp =
			UnifiedSendResponse { payment_result: Some(PaymentResult::Txid("tx".to_string())) };
		assert!(matches!(
			UnifiedSendResult::try_from(txid_resp).unwrap(),
			UnifiedSendResult::Onchain { .. }
		));

		let b11_resp = UnifiedSendResponse {
			payment_result: Some(PaymentResult::Bolt11PaymentId("p1".to_string())),
		};
		assert!(matches!(
			UnifiedSendResult::try_from(b11_resp).unwrap(),
			UnifiedSendResult::Bolt11 { .. }
		));

		let b12_resp = UnifiedSendResponse {
			payment_result: Some(PaymentResult::Bolt12PaymentId("p2".to_string())),
		};
		assert!(matches!(
			UnifiedSendResult::try_from(b12_resp).unwrap(),
			UnifiedSendResult::Bolt12 { .. }
		));

		// Empty payment_result is a protocol violation and should surface as an error rather
		// than a mystery default.
		let empty = UnifiedSendResponse { payment_result: None };
		assert!(matches!(
			UnifiedSendResult::try_from(empty),
			Err(LdkServerClientError::InternalError { .. })
		));
	}

	#[test]
	fn bolt11_receive_roundtrip() {
		let resp = Bolt11ReceiveResponse {
			invoice: "lnbc...".to_string(),
			payment_hash: "ph".to_string(),
			payment_secret: "ps".to_string(),
		};
		let result: Bolt11ReceiveResult = resp.into();
		assert_eq!(result.invoice, "lnbc...");
		assert_eq!(result.payment_hash, "ph");
		assert_eq!(result.payment_secret, "ps");
	}

	#[test]
	fn decoded_offer_extracts_bitcoin_amount_only() {
		let with_btc = DecodeOfferResponse {
			offer_id: "oid".to_string(),
			description: None,
			issuer: None,
			amount: Some(OfferAmount { amount: Some(OfferAmountOneof::BitcoinAmountMsats(5_000)) }),
			issuer_signing_pubkey: None,
			absolute_expiry: None,
			quantity: None,
			paths: vec![],
			features: Default::default(),
			chains: vec![],
			metadata: None,
			is_expired: false,
		};
		let d: DecodedOffer = with_btc.into();
		assert_eq!(d.amount_msat, Some(5_000));

		// Currency-denominated offers flatten to None, not an error — a wallet that can only
		// pay in Bitcoin should treat this the same as an amount-less offer.
		let with_currency = DecodeOfferResponse {
			offer_id: "oid".to_string(),
			description: None,
			issuer: None,
			amount: Some(OfferAmount {
				amount: Some(OfferAmountOneof::CurrencyAmount(CurrencyAmount {
					iso4217_code: "USD".to_string(),
					amount: 42,
				})),
			}),
			issuer_signing_pubkey: None,
			absolute_expiry: None,
			quantity: None,
			paths: vec![],
			features: Default::default(),
			chains: vec![],
			metadata: None,
			is_expired: false,
		};
		let d: DecodedOffer = with_currency.into();
		assert_eq!(d.amount_msat, None);
	}

	#[test]
	fn decoded_invoice_roundtrip() {
		let resp = DecodeInvoiceResponse {
			destination: "d".to_string(),
			payment_hash: "ph".to_string(),
			amount_msat: Some(1_000),
			timestamp: 1,
			expiry: 2,
			description: Some("coffee".to_string()),
			description_hash: None,
			fallback_address: None,
			min_final_cltv_expiry_delta: 3,
			payment_secret: "ps".to_string(),
			route_hints: vec![],
			features: Default::default(),
			currency: "bitcoin".to_string(),
			payment_metadata: None,
			is_expired: false,
		};
		let d: DecodedInvoice = resp.into();
		assert_eq!(d.amount_msat, Some(1_000));
		assert_eq!(d.description.as_deref(), Some("coffee"));
		assert_eq!(d.currency, "bitcoin");
	}

	#[test]
	fn page_token_roundtrips_both_ways() {
		let prost = PageToken { token: "t".to_string(), index: 5 };
		let info: PageTokenInfo = prost.clone().into();
		assert_eq!(info.index, 5);
		let back: PageToken = info.into();
		assert_eq!(back.token, "t");
		assert_eq!(back.index, 5);
	}
}
