// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use serde::{Deserialize, Serialize};

/// Represents a payment.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.PaymentDetails.html>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Payment {
	/// An identifier used to uniquely identify a payment in hex-encoded form.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub id: [u8; 32],
	/// The kind of the payment.
	pub kind: PaymentKind,
	/// The amount transferred.
	pub amount_msat: Option<u64>,
	/// The fees that were paid for this payment.
	///
	/// For Lightning payments, this will only be updated for outbound payments once they
	/// succeeded.
	pub fee_paid_msat: Option<u64>,
	/// The direction of the payment.
	pub direction: PaymentDirection,
	/// The status of the payment.
	pub status: PaymentStatus,
	/// The timestamp, in seconds since start of the UNIX epoch, when this entry was last updated.
	pub latest_update_timestamp: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentKind {
	Onchain(Onchain),
	Bolt11(Bolt11),
	Bolt11Jit(Bolt11Jit),
	Bolt12Offer(Bolt12Offer),
	Bolt12Refund(Bolt12Refund),
	Spontaneous(Spontaneous),
}
/// Represents an on-chain payment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Onchain {
	/// The transaction identifier of this payment.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub txid: [u8; 32],
	/// The confirmation status of this payment.
	pub status: ConfirmationStatus,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationStatus {
	Confirmed(Confirmed),
	Unconfirmed(Unconfirmed),
}
/// The on-chain transaction is confirmed in the best chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Confirmed {
	/// The hex representation of hash of the block in which the transaction was confirmed.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub block_hash: [u8; 32],
	/// The height under which the block was confirmed.
	pub height: u32,
	/// The timestamp, in seconds since start of the UNIX epoch, when this entry was last updated.
	pub timestamp: u64,
}
/// The on-chain transaction is unconfirmed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Unconfirmed {}
/// Represents a BOLT 11 payment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt11 {
	/// The payment hash, i.e., the hash of the preimage.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub hash: [u8; 32],
	/// The pre-image used by the payment.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub preimage: Option<[u8; 32]>,
	/// The secret used by the payment.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub secret: Option<[u8; 32]>,
}
/// Represents a BOLT 11 payment intended to open an LSPS 2 just-in-time channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt11Jit {
	/// The payment hash, i.e., the hash of the preimage.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub hash: [u8; 32],
	/// The pre-image used by the payment.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub preimage: Option<[u8; 32]>,
	/// The secret used by the payment.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub secret: Option<[u8; 32]>,
	/// Limits applying to how much fee we allow an LSP to deduct from the payment amount.
	///
	/// Allowing them to deduct this fee from the first inbound payment will pay for the LSP's channel opening fees.
	///
	/// See \[`LdkChannelConfig::accept_underpaying_htlcs`\](<https://docs.rs/lightning/latest/lightning/util/config/struct.ChannelConfig.html#structfield.accept_underpaying_htlcs>)
	/// for more information.
	pub lsp_fee_limits: Option<LspFeeLimits>,
	/// The value, in thousands of a satoshi, that was deducted from this payment as an extra
	/// fee taken by our channel counterparty.
	///
	/// Will only be `Some` once we received the payment.
	pub counterparty_skimmed_fee_msat: Option<u64>,
}
/// Represents a BOLT 12 'offer' payment, i.e., a payment for an Offer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt12Offer {
	/// The payment hash, i.e., the hash of the preimage.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub hash: Option<[u8; 32]>,
	/// The pre-image used by the payment.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub preimage: Option<[u8; 32]>,
	/// The secret used by the payment.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub secret: Option<[u8; 32]>,
	/// The hex-encoded ID of the offer this payment is for.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub offer_id: [u8; 32],
	/// The payer's note for the payment.
	/// Truncated to \[PAYER_NOTE_LIMIT\](<https://docs.rs/lightning/latest/lightning/offers/invoice_request/constant.PAYER_NOTE_LIMIT.html>).
	///
	/// **Caution**: The `payer_note` field may come from an untrusted source. To prevent potential misuse,
	/// all non-printable characters will be sanitized and replaced with safe characters.
	pub payer_note: Option<String>,
	/// The quantity of an item requested in the offer.
	pub quantity: Option<u64>,
}
/// Represents a BOLT 12 'refund' payment, i.e., a payment for a Refund.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt12Refund {
	/// The payment hash, i.e., the hash of the preimage.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub hash: Option<[u8; 32]>,
	/// The pre-image used by the payment.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub preimage: Option<[u8; 32]>,
	/// The secret used by the payment.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub secret: Option<[u8; 32]>,
	/// The payer's note for the payment.
	/// Truncated to \[PAYER_NOTE_LIMIT\](<https://docs.rs/lightning/latest/lightning/offers/invoice_request/constant.PAYER_NOTE_LIMIT.html>).
	///
	/// **Caution**: The `payer_note` field may come from an untrusted source. To prevent potential misuse,
	/// all non-printable characters will be sanitized and replaced with safe characters.
	pub payer_note: Option<String>,
	/// The quantity of an item requested in the offer.
	pub quantity: Option<u64>,
}
/// Represents a spontaneous ("keysend") payment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spontaneous {
	/// The payment hash, i.e., the hash of the preimage.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub hash: [u8; 32],
	/// The pre-image used by the payment.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub preimage: Option<[u8; 32]>,
}
/// Limits applying to how much fee we allow an LSP to deduct from the payment amount.
/// See \[`LdkChannelConfig::accept_underpaying_htlcs`\] for more information.
///
/// \[`LdkChannelConfig::accept_underpaying_htlcs`\]: lightning::util::config::ChannelConfig::accept_underpaying_htlcs
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LspFeeLimits {
	/// The maximal total amount we allow any configured LSP withhold from us when forwarding the
	/// payment.
	pub max_total_opening_fee_msat: Option<u64>,
	/// The maximal proportional fee, in parts-per-million millisatoshi, we allow any configured
	/// LSP withhold from us when forwarding the payment.
	pub max_proportional_opening_fee_ppm_msat: Option<u64>,
}
/// A forwarded payment through our node.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/enum.Event.html#variant.PaymentForwarded>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForwardedPayment {
	/// The channel id of the incoming channel between the previous node and us.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub prev_channel_id: [u8; 32],
	/// The channel id of the outgoing channel between the next node and us.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub next_channel_id: [u8; 32],
	/// The `user_channel_id` of the incoming channel between the previous node and us.
	pub prev_user_channel_id: String,
	/// The node id of the previous node.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub prev_node_id: [u8; 33],
	/// The node id of the next node.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub next_node_id: [u8; 33],
	/// The `user_channel_id` of the outgoing channel between the next node and us.
	/// This will be `None` if the payment was settled via an on-chain transaction.
	/// See the caveat described for the `total_fee_earned_msat` field.
	pub next_user_channel_id: Option<String>,
	/// The total fee, in milli-satoshis, which was earned as a result of the payment.
	///
	/// Note that if we force-closed the channel over which we forwarded an HTLC while the HTLC was pending, the amount the
	/// next hop claimed will have been rounded down to the nearest whole satoshi. Thus, the fee calculated here may be
	/// higher than expected as we still claimed the full value in millisatoshis from the source.
	/// In this case, `claim_from_onchain_tx` will be set.
	///
	/// If the channel which sent us the payment has been force-closed, we will claim the funds via an on-chain transaction.
	/// In that case we do not yet know the on-chain transaction fees which we will spend and will instead set this to `None`.
	pub total_fee_earned_msat: Option<u64>,
	/// The share of the total fee, in milli-satoshis, which was withheld in addition to the forwarding fee.
	/// This will only be set if we forwarded an intercepted HTLC with less than the expected amount. This means our
	/// counterparty accepted to receive less than the invoice amount.
	///
	/// The caveat described above the `total_fee_earned_msat` field applies here as well.
	pub skimmed_fee_msat: Option<u64>,
	/// If this is true, the forwarded HTLC was claimed by our counterparty via an on-chain transaction.
	pub claim_from_onchain_tx: bool,
	/// The final amount forwarded, in milli-satoshis, after the fee is deducted.
	///
	/// The caveat described above the `total_fee_earned_msat` field applies here as well.
	pub outbound_amount_forwarded_msat: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel {
	/// The channel ID (prior to funding transaction generation, this is a random 32-byte
	/// identifier, afterwards this is the transaction ID of the funding transaction XOR the
	/// funding transaction output).
	///
	/// Note that this means this value is *not* persistent - it can change once during the
	/// lifetime of the channel.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub channel_id: [u8; 32],
	/// The node ID of our the channel's remote counterparty.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub counterparty_node_id: [u8; 33],
	/// The channel's funding transaction output, if we've negotiated the funding transaction with
	/// our counterparty already.
	pub funding_txo: Option<OutPoint>,
	/// The hex-encoded local `user_channel_id` of this channel.
	pub user_channel_id: String,
	/// The value, in satoshis, that must always be held as a reserve in the channel for us. This
	/// value ensures that if we broadcast a revoked state, our counterparty can punish us by
	/// claiming at least this value on chain.
	///
	/// This value is not included in \[`outbound_capacity_msat`\] as it can never be spent.
	///
	/// This value will be `None` for outbound channels until the counterparty accepts the channel.
	pub unspendable_punishment_reserve: Option<u64>,
	/// The value, in satoshis, of this channel as it appears in the funding output.
	pub channel_value_sats: u64,
	/// The currently negotiated fee rate denominated in satoshi per 1000 weight units,
	/// which is applied to commitment and HTLC transactions.
	pub feerate_sat_per_1000_weight: u32,
	/// The available outbound capacity for sending HTLCs to the remote peer.
	///
	/// The amount does not include any pending HTLCs which are not yet resolved (and, thus, whose
	/// balance is not available for inclusion in new outbound HTLCs). This further does not include
	/// any pending outgoing HTLCs which are awaiting some other resolution to be sent.
	pub outbound_capacity_msat: u64,
	/// The available outbound capacity for sending HTLCs to the remote peer.
	///
	/// The amount does not include any pending HTLCs which are not yet resolved
	/// (and, thus, whose balance is not available for inclusion in new inbound HTLCs). This further
	/// does not include any pending outgoing HTLCs which are awaiting some other resolution to be
	/// sent.
	pub inbound_capacity_msat: u64,
	/// The number of required confirmations on the funding transactions before the funding is
	/// considered "locked". The amount is selected by the channel fundee.
	///
	/// The value will be `None` for outbound channels until the counterparty accepts the channel.
	pub confirmations_required: Option<u32>,
	/// The current number of confirmations on the funding transaction.
	pub confirmations: Option<u32>,
	/// Is `true` if the channel was initiated (and therefore funded) by us.
	pub is_outbound: bool,
	/// Is `true` if both parties have exchanged `channel_ready` messages, and the channel is
	/// not currently being shut down. Both parties exchange `channel_ready` messages upon
	/// independently verifying that the required confirmations count provided by
	/// `confirmations_required` has been reached.
	pub is_channel_ready: bool,
	/// Is `true` if the channel (a) `channel_ready` messages have been exchanged, (b) the
	/// peer is connected, and (c) the channel is not currently negotiating shutdown.
	///
	/// This is a strict superset of `is_channel_ready`.
	pub is_usable: bool,
	/// Is `true` if this channel is (or will be) publicly-announced
	pub is_announced: bool,
	/// Set of configurable parameters set by self that affect channel operation.
	pub channel_config: Option<ChannelConfig>,
	/// The available outbound capacity for sending a single HTLC to the remote peer. This is
	/// similar to `outbound_capacity_msat` but it may be further restricted by
	/// the current state and per-HTLC limit(s). This is intended for use when routing, allowing us
	/// to use a limit as close as possible to the HTLC limit we can currently send.
	pub next_outbound_htlc_limit_msat: u64,
	/// The minimum value for sending a single HTLC to the remote peer. This is the equivalent of
	/// `next_outbound_htlc_limit_msat` but represents a lower-bound, rather than
	/// an upper-bound. This is intended for use when routing, allowing us to ensure we pick a
	/// route which is valid.
	pub next_outbound_htlc_minimum_msat: u64,
	/// The number of blocks (after our commitment transaction confirms) that we will need to wait
	/// until we can claim our funds after we force-close the channel. During this time our
	/// counterparty is allowed to punish us if we broadcasted a stale state. If our counterparty
	/// force-closes the channel and broadcasts a commitment transaction we do not have to wait any
	/// time to claim our non-HTLC-encumbered funds.
	///
	/// This value will be `None` for outbound channels until the counterparty accepts the channel.
	pub force_close_spend_delay: Option<u32>,
	/// The smallest value HTLC (in msat) the remote peer will accept, for this channel.
	///
	/// This field is only `None` before we have received either the `OpenChannel` or
	/// `AcceptChannel` message from the remote peer.
	pub counterparty_outbound_htlc_minimum_msat: Option<u64>,
	/// The largest value HTLC (in msat) the remote peer currently will accept, for this channel.
	pub counterparty_outbound_htlc_maximum_msat: Option<u64>,
	/// The value, in satoshis, that must always be held in the channel for our counterparty. This
	/// value ensures that if our counterparty broadcasts a revoked state, we can punish them by
	/// claiming at least this value on chain.
	///
	/// This value is not included in `inbound_capacity_msat` as it can never be spent.
	pub counterparty_unspendable_punishment_reserve: u64,
	/// Base routing fee in millisatoshis.
	pub counterparty_forwarding_info_fee_base_msat: Option<u32>,
	/// Proportional fee, in millionths of a satoshi the channel will charge per transferred satoshi.
	pub counterparty_forwarding_info_fee_proportional_millionths: Option<u32>,
	/// The minimum difference in CLTV expiry between an ingoing HTLC and its outgoing counterpart,
	/// such that the outgoing HTLC is forwardable to this counterparty.
	pub counterparty_forwarding_info_cltv_expiry_delta: Option<u32>,
}
/// ChannelConfig represents the configuration settings for a channel in a Lightning Network node.
/// See more: <https://docs.rs/lightning/latest/lightning/util/config/struct.ChannelConfig.html>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelConfig {
	/// Amount (in millionths of a satoshi) charged per satoshi for payments forwarded outbound
	/// over the channel.
	/// See more: <https://docs.rs/lightning/latest/lightning/util/config/struct.ChannelConfig.html#structfield.forwarding_fee_proportional_millionths>
	pub forwarding_fee_proportional_millionths: Option<u32>,
	/// Amount (in milli-satoshi) charged for payments forwarded outbound over the channel,
	/// in excess of forwarding_fee_proportional_millionths.
	/// See more: <https://docs.rs/lightning/latest/lightning/util/config/struct.ChannelConfig.html#structfield.forwarding_fee_base_msat>
	pub forwarding_fee_base_msat: Option<u32>,
	/// The difference in the CLTV value between incoming HTLCs and an outbound HTLC forwarded
	/// over the channel this config applies to.
	/// See more: <https://docs.rs/lightning/latest/lightning/util/config/struct.ChannelConfig.html#structfield.cltv_expiry_delta>
	pub cltv_expiry_delta: Option<u32>,
	/// The maximum additional fee we're willing to pay to avoid waiting for the counterparty's
	/// to_self_delay to reclaim funds.
	/// See more: <https://docs.rs/lightning/latest/lightning/util/config/struct.ChannelConfig.html#structfield.force_close_avoidance_max_fee_satoshis>
	pub force_close_avoidance_max_fee_satoshis: Option<u64>,
	/// If set, allows this channel's counterparty to skim an additional fee off this node's
	/// inbound HTLCs. Useful for liquidity providers to offload on-chain channel costs to end users.
	/// See more: <https://docs.rs/lightning/latest/lightning/util/config/struct.ChannelConfig.html#structfield.accept_underpaying_htlcs>
	pub accept_underpaying_htlcs: Option<bool>,
	/// Limit our total exposure to potential loss to on-chain fees on close, including
	/// in-flight HTLCs which are burned to fees as they are too small to claim on-chain
	/// and fees on commitment transaction(s) broadcasted by our counterparty in excess of
	/// our own fee estimate.
	/// See more: <https://docs.rs/lightning/latest/lightning/util/config/struct.ChannelConfig.html#structfield.max_dust_htlc_exposure>
	pub max_dust_htlc_exposure: Option<MaxDustHtlcExposure>,
}
/// Limit our total exposure to potential loss to on-chain fees on close, including
/// in-flight HTLCs which are burned to fees as they are too small to claim on-chain
/// and fees on commitment transaction(s) broadcasted by our counterparty in excess of
/// our own fee estimate.
/// See more: <https://docs.rs/lightning/latest/lightning/util/config/struct.ChannelConfig.html#structfield.max_dust_htlc_exposure>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaxDustHtlcExposure {
	/// This sets a fixed limit on the total dust exposure in millisatoshis.
	/// See more: <https://docs.rs/lightning/latest/lightning/util/config/enum.MaxDustHTLCExposure.html#variant.FixedLimitMsat>
	FixedLimitMsat(u64),
	/// This sets a multiplier on the ConfirmationTarget::OnChainSweep feerate (in sats/KW) to determine the maximum allowed dust exposure.
	/// See more: <https://docs.rs/lightning/latest/lightning/util/config/enum.MaxDustHTLCExposure.html#variant.FeeRateMultiplier>
	FeeRateMultiplier(u64),
}
/// Represent a transaction outpoint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutPoint {
	/// The referenced transaction's txid.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub txid: [u8; 32],
	/// The index of the referenced output in its transaction's vout.
	pub vout: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BestBlock {
	/// The block's hash
	#[serde(with = "crate::serde_utils::hex_32")]
	pub block_hash: [u8; 32],
	/// The height at which the block was confirmed.
	pub height: u32,
}
/// Details about the status of a known Lightning balance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightningBalance {
	ClaimableOnChannelClose(ClaimableOnChannelClose),
	ClaimableAwaitingConfirmations(ClaimableAwaitingConfirmations),
	ContentiousClaimable(ContentiousClaimable),
	MaybeTimeoutClaimableHtlc(MaybeTimeoutClaimableHtlc),
	MaybePreimageClaimableHtlc(MaybePreimageClaimableHtlc),
	CounterpartyRevokedOutputClaimable(CounterpartyRevokedOutputClaimable),
}
/// The channel is not yet closed (or the commitment or closing transaction has not yet appeared in a block).
/// The given balance is claimable (less on-chain fees) if the channel is force-closed now.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/enum.LightningBalance.html#variant.ClaimableOnChannelClose>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClaimableOnChannelClose {
	/// The identifier of the channel this balance belongs to.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub channel_id: [u8; 32],
	/// The identifier of our channel counterparty.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub counterparty_node_id: [u8; 33],
	/// The amount available to claim, in satoshis, excluding the on-chain fees which will be required to do so.
	pub amount_satoshis: u64,
	/// The transaction fee we pay for the closing commitment transaction.
	/// This amount is not included in the `amount_satoshis` value.
	///
	/// Note that if this channel is inbound (and thus our counterparty pays the commitment transaction fee) this value
	/// will be zero.
	pub transaction_fee_satoshis: u64,
	/// The amount of millisatoshis which has been burned to fees from HTLCs which are outbound from us and are related to
	/// a payment which was sent by us. This is the sum of the millisatoshis part of all HTLCs which are otherwise
	/// represented by `LightningBalance::MaybeTimeoutClaimableHTLC` with their
	/// `LightningBalance::MaybeTimeoutClaimableHTLC::outbound_payment` flag set, as well as any dust HTLCs which would
	/// otherwise be represented the same.
	///
	/// This amount (rounded up to a whole satoshi value) will not be included in `amount_satoshis`.
	pub outbound_payment_htlc_rounded_msat: u64,
	/// The amount of millisatoshis which has been burned to fees from HTLCs which are outbound from us and are related to
	/// a forwarded HTLC. This is the sum of the millisatoshis part of all HTLCs which are otherwise represented by
	/// `LightningBalance::MaybeTimeoutClaimableHTLC` with their `LightningBalance::MaybeTimeoutClaimableHTLC::outbound_payment`
	/// flag not set, as well as any dust HTLCs which would otherwise be represented the same.
	///
	/// This amount (rounded up to a whole satoshi value) will not be included in `amount_satoshis`.
	pub outbound_forwarded_htlc_rounded_msat: u64,
	/// The amount of millisatoshis which has been burned to fees from HTLCs which are inbound to us and for which we know
	/// the preimage. This is the sum of the millisatoshis part of all HTLCs which would be represented by
	/// `LightningBalance::ContentiousClaimable` on channel close, but whose current value is included in `amount_satoshis`,
	/// as well as any dust HTLCs which would otherwise be represented the same.
	///
	/// This amount (rounded up to a whole satoshi value) will not be included in `amount_satoshis`.
	pub inbound_claiming_htlc_rounded_msat: u64,
	/// The amount of millisatoshis which has been burned to fees from HTLCs which are inbound to us and for which we do
	/// not know the preimage. This is the sum of the millisatoshis part of all HTLCs which would be represented by
	/// `LightningBalance::MaybePreimageClaimableHTLC` on channel close, as well as any dust HTLCs which would otherwise be
	/// represented the same.
	///
	/// This amount (rounded up to a whole satoshi value) will not be included in the counterparty's `amount_satoshis`.
	pub inbound_htlc_rounded_msat: u64,
}
/// The channel has been closed, and the given balance is ours but awaiting confirmations until we consider it spendable.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/enum.LightningBalance.html#variant.ClaimableAwaitingConfirmations>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClaimableAwaitingConfirmations {
	/// The identifier of the channel this balance belongs to.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub channel_id: [u8; 32],
	/// The identifier of our channel counterparty.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub counterparty_node_id: [u8; 33],
	/// The amount available to claim, in satoshis, possibly excluding the on-chain fees which were spent in broadcasting
	/// the transaction.
	pub amount_satoshis: u64,
	/// The height at which we start tracking it as  `SpendableOutput`.
	pub confirmation_height: u32,
	/// Whether this balance is a result of cooperative close, a force-close, or an HTLC.
	pub source: BalanceSource,
}
/// The channel has been closed, and the given balance should be ours but awaiting spending transaction confirmation.
/// If the spending transaction does not confirm in time, it is possible our counterparty can take the funds by
/// broadcasting an HTLC timeout on-chain.
///
/// Once the spending transaction confirms, before it has reached enough confirmations to be considered safe from chain
/// reorganizations, the balance will instead be provided via `LightningBalance::ClaimableAwaitingConfirmations`.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/enum.LightningBalance.html#variant.ContentiousClaimable>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContentiousClaimable {
	/// The identifier of the channel this balance belongs to.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub channel_id: [u8; 32],
	/// The identifier of our channel counterparty.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub counterparty_node_id: [u8; 33],
	/// The amount available to claim, in satoshis, excluding the on-chain fees which were spent in broadcasting
	/// the transaction.
	pub amount_satoshis: u64,
	/// The height at which the counterparty may be able to claim the balance if we have not done so.
	pub timeout_height: u32,
	/// The payment hash that locks this HTLC.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub payment_hash: [u8; 32],
	/// The preimage that can be used to claim this HTLC.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub payment_preimage: [u8; 32],
}
/// HTLCs which we sent to our counterparty which are claimable after a timeout (less on-chain fees) if the counterparty
/// does not know the preimage for the HTLCs. These are somewhat likely to be claimed by our counterparty before we do.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/enum.LightningBalance.html#variant.MaybeTimeoutClaimableHTLC>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaybeTimeoutClaimableHtlc {
	/// The identifier of the channel this balance belongs to.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub channel_id: [u8; 32],
	/// The identifier of our channel counterparty.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub counterparty_node_id: [u8; 33],
	/// The amount available to claim, in satoshis, excluding the on-chain fees which were spent in broadcasting
	/// the transaction.
	pub amount_satoshis: u64,
	/// The height at which we will be able to claim the balance if our counterparty has not done so.
	pub claimable_height: u32,
	/// The payment hash whose preimage our counterparty needs to claim this HTLC.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub payment_hash: [u8; 32],
	/// Indicates whether this HTLC represents a payment which was sent outbound from us.
	pub outbound_payment: bool,
}
/// HTLCs which we received from our counterparty which are claimable with a preimage which we do not currently have.
/// This will only be claimable if we receive the preimage from the node to which we forwarded this HTLC before the
/// timeout.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/enum.LightningBalance.html#variant.MaybePreimageClaimableHTLC>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaybePreimageClaimableHtlc {
	/// The identifier of the channel this balance belongs to.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub channel_id: [u8; 32],
	/// The identifier of our channel counterparty.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub counterparty_node_id: [u8; 33],
	/// The amount available to claim, in satoshis, excluding the on-chain fees which were spent in broadcasting
	/// the transaction.
	pub amount_satoshis: u64,
	/// The height at which our counterparty will be able to claim the balance if we have not yet received the preimage and
	/// claimed it ourselves.
	pub expiry_height: u32,
	/// The payment hash whose preimage we need to claim this HTLC.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub payment_hash: [u8; 32],
}
/// The channel has been closed, and our counterparty broadcasted a revoked commitment transaction.
///
/// Thus, we're able to claim all outputs in the commitment transaction, one of which has the following amount.
///
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/enum.LightningBalance.html#variant.CounterpartyRevokedOutputClaimable>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CounterpartyRevokedOutputClaimable {
	/// The identifier of the channel this balance belongs to.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub channel_id: [u8; 32],
	/// The identifier of our channel counterparty.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub counterparty_node_id: [u8; 33],
	/// The amount, in satoshis, of the output which we can claim.
	pub amount_satoshis: u64,
}
/// Details about the status of a known balance currently being swept to our on-chain wallet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingSweepBalance {
	PendingBroadcast(PendingBroadcast),
	BroadcastAwaitingConfirmation(BroadcastAwaitingConfirmation),
	AwaitingThresholdConfirmations(AwaitingThresholdConfirmations),
}
/// The spendable output is about to be swept, but a spending transaction has yet to be generated and broadcast.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/enum.PendingSweepBalance.html#variant.PendingBroadcast>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingBroadcast {
	/// The identifier of the channel this balance belongs to.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub channel_id: Option<[u8; 32]>,
	/// The amount, in satoshis, of the output being swept.
	pub amount_satoshis: u64,
}
/// A spending transaction has been generated and broadcast and is awaiting confirmation on-chain.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/enum.PendingSweepBalance.html#variant.BroadcastAwaitingConfirmation>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BroadcastAwaitingConfirmation {
	/// The identifier of the channel this balance belongs to.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub channel_id: Option<[u8; 32]>,
	/// The best height when we last broadcast a transaction spending the output being swept.
	pub latest_broadcast_height: u32,
	/// The identifier of the transaction spending the swept output we last broadcast.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub latest_spending_txid: [u8; 32],
	/// The amount, in satoshis, of the output being swept.
	pub amount_satoshis: u64,
}
/// A spending transaction has been confirmed on-chain and is awaiting threshold confirmations.
///
/// It will be considered irrevocably confirmed after reaching `ANTI_REORG_DELAY`.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/enum.PendingSweepBalance.html#variant.AwaitingThresholdConfirmations>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AwaitingThresholdConfirmations {
	/// The identifier of the channel this balance belongs to.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub channel_id: Option<[u8; 32]>,
	/// The identifier of the confirmed transaction spending the swept output.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub latest_spending_txid: [u8; 32],
	/// The hash of the block in which the spending transaction was confirmed.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub confirmation_hash: [u8; 32],
	/// The height at which the spending transaction was confirmed.
	pub confirmation_height: u32,
	/// The amount, in satoshis, of the output being swept.
	pub amount_satoshis: u64,
}
/// Token used to determine start of next page in paginated APIs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageToken {
	pub token: String,
	pub index: i64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bolt11InvoiceDescription {
	Direct(String),
	Hash(String),
}
/// Configuration options for payment routing and pathfinding.
/// See <https://docs.rs/lightning/0.2.0/lightning/routing/router/struct.RouteParametersConfig.html> for more details on each field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouteParametersConfig {
	/// The maximum total fees, in millisatoshi, that may accrue during route finding.
	/// Defaults to 1% of the payment amount + 50 sats
	pub max_total_routing_fee_msat: Option<u64>,
	/// The maximum total CLTV delta we accept for the route.
	/// Defaults to 1008.
	pub max_total_cltv_expiry_delta: u32,
	/// The maximum number of paths that may be used by (MPP) payments.
	/// Defaults to 10.
	pub max_path_count: u32,
	/// Selects the maximum share of a channel's total capacity which will be
	/// sent over a channel, as a power of 1/2.
	/// Default value: 2
	pub max_channel_saturation_power_of_half: u32,
}
/// Routing fees for a channel as part of the network graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphRoutingFees {
	/// Flat routing fee in millisatoshis.
	pub base_msat: u32,
	/// Liquidity-based routing fee in millionths of a routed amount.
	pub proportional_millionths: u32,
}
/// Details about one direction of a channel in the network graph,
/// as received within a `ChannelUpdate`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphChannelUpdate {
	/// When the last update to the channel direction was issued.
	/// Value is opaque, as set in the announcement.
	pub last_update: u32,
	/// Whether the channel can be currently used for payments (in this one direction).
	pub enabled: bool,
	/// The difference in CLTV values that you must have when routing through this channel.
	pub cltv_expiry_delta: u32,
	/// The minimum value, which must be relayed to the next hop via the channel.
	pub htlc_minimum_msat: u64,
	/// The maximum value which may be relayed to the next hop via the channel.
	pub htlc_maximum_msat: u64,
	/// Fees charged when the channel is used for routing.
	pub fees: GraphRoutingFees,
}
/// Details about a channel in the network graph (both directions).
/// Received within a channel announcement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphChannel {
	/// Source node of the first direction of the channel (hex-encoded public key).
	#[serde(with = "crate::serde_utils::hex_33")]
	pub node_one: [u8; 33],
	/// Source node of the second direction of the channel (hex-encoded public key).
	#[serde(with = "crate::serde_utils::hex_33")]
	pub node_two: [u8; 33],
	/// The channel capacity as seen on-chain, if chain lookup is available.
	pub capacity_sats: Option<u64>,
	/// Details about the first direction of a channel.
	pub one_to_two: Option<GraphChannelUpdate>,
	/// Details about the second direction of a channel.
	pub two_to_one: Option<GraphChannelUpdate>,
}
/// Information received in the latest node_announcement from this node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphNodeAnnouncement {
	/// When the last known update to the node state was issued.
	/// Value is opaque, as set in the announcement.
	pub last_update: u32,
	/// Moniker assigned to the node.
	/// May be invalid or malicious (eg control chars), should not be exposed to the user.
	pub alias: String,
	/// Color assigned to the node as a hex-encoded RGB string, e.g. "ff0000".
	pub rgb: String,
	/// List of addresses on which this node is reachable.
	pub addresses: Vec<String>,
}
/// Details of a known Lightning peer.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.list_peers>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Peer {
	/// The hex-encoded node ID of the peer.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub node_id: [u8; 33],
	/// The network address of the peer.
	pub address: String,
	/// Indicates whether we'll try to reconnect to this peer after restarts.
	pub is_persisted: bool,
	/// Indicates whether we currently have an active connection with the peer.
	pub is_connected: bool,
}
/// Details about a node in the network graph, known from the network announcement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
	/// All valid channels a node has announced.
	pub channels: Vec<u64>,
	/// More information about a node from node_announcement.
	/// Optional because we store a node entry after learning about it from
	/// a channel announcement, but before receiving a node announcement.
	pub announcement_info: Option<GraphNodeAnnouncement>,
}
/// Represents the direction of a payment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentDirection {
	/// The payment is inbound.
	Inbound,
	/// The payment is outbound.
	Outbound,
}
/// Represents the current status of a payment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
	/// The payment is still pending.
	Pending,
	/// The payment succeeded.
	Succeeded,
	/// The payment failed.
	Failed,
}
/// Indicates whether the balance is derived from a cooperative close, a force-close (for holder or counterparty),
/// or whether it is for an HTLC.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BalanceSource {
	/// The channel was force closed by the holder.
	HolderForceClosed,
	/// The channel was force closed by the counterparty.
	CounterpartyForceClosed,
	/// The channel was cooperatively closed.
	CoopClose,
	/// This balance is the result of an HTLC.
	Htlc,
}
