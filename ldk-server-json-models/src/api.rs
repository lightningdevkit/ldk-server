// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use serde::{Deserialize, Serialize};

/// Retrieve the latest node info like `node_id`, `current_best_block` etc.
/// See more:
/// - <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.node_id>
/// - <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.status>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GetNodeInfoRequest {}
/// The response `content` for the `GetNodeInfo` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GetNodeInfoResponse {
	/// The hex-encoded `node-id` or public key for our own lightning node.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub node_id: [u8; 33],
	/// The best block to which our Lightning wallet is currently synced.
	pub current_best_block: super::types::BestBlock,
	/// The timestamp, in seconds since start of the UNIX epoch, when we last successfully synced our Lightning wallet to
	/// the chain tip.
	///
	/// Will be `None` if the wallet hasn't been synced yet.
	pub latest_lightning_wallet_sync_timestamp: Option<u64>,
	/// The timestamp, in seconds since start of the UNIX epoch, when we last successfully synced our on-chain
	/// wallet to the chain tip.
	///
	/// Will be `None` if the wallet hasn't been synced since the node was initialized.
	pub latest_onchain_wallet_sync_timestamp: Option<u64>,
	/// The timestamp, in seconds since start of the UNIX epoch, when we last successfully update our fee rate cache.
	///
	/// Will be `None` if the cache hasn't been updated since the node was initialized.
	pub latest_fee_rate_cache_update_timestamp: Option<u64>,
	/// The timestamp, in seconds since start of the UNIX epoch, when the last rapid gossip sync (RGS) snapshot we
	/// successfully applied was generated.
	///
	/// Will be `None` if RGS isn't configured or the snapshot hasn't been updated since the node was initialized.
	pub latest_rgs_snapshot_timestamp: Option<u64>,
	/// The timestamp, in seconds since start of the UNIX epoch, when we last broadcasted a node announcement.
	///
	/// Will be `None` if we have no public channels or we haven't broadcasted since the node was initialized.
	pub latest_node_announcement_broadcast_timestamp: Option<u64>,
	/// The addresses the node is currently listening on for incoming connections.
	///
	/// Will be empty if the node is not listening on any addresses.
	pub listening_addresses: Vec<String>,
	/// The addresses the node announces to the network.
	///
	/// Will be empty if no announcement addresses are configured.
	pub announcement_addresses: Vec<String>,
	/// The node alias, if configured.
	///
	/// Will be `None` if no alias is configured.
	pub node_alias: Option<String>,
	/// The node URIs that can be used to connect to this node, in the format `node_id@address`.
	///
	/// These are constructed from the announcement addresses and the node's public key.
	/// Will be empty if no announcement addresses are configured.
	pub node_uris: Vec<String>,
}
/// Retrieve a new on-chain funding address.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.OnchainPayment.html#method.new_address>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OnchainReceiveRequest {}
/// The response `content` for the `OnchainReceive` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`..
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OnchainReceiveResponse {
	/// A Bitcoin on-chain address.
	pub address: String,
}
/// Send an on-chain payment to the given address.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OnchainSendRequest {
	/// The address to send coins to.
	pub address: String,
	/// The amount in satoshis to send.
	/// While sending the specified amount, we will respect any on-chain reserve we need to keep,
	/// i.e., won't allow to cut into `total_anchor_channels_reserve_sats`.
	/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.OnchainPayment.html#method.send_to_address>
	pub amount_sats: Option<u64>,
	/// If set, the amount_sats field should be unset.
	/// It indicates that node will send full balance to the specified address.
	///
	/// Please note that when send_all is used this operation will **not** retain any on-chain reserves,
	/// which might be potentially dangerous if you have open Anchor channels for which you can't trust
	/// the counterparty to spend the Anchor output after channel closure.
	/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.OnchainPayment.html#method.send_all_to_address>
	pub send_all: Option<bool>,
	/// If `fee_rate_sat_per_vb` is set it will be used on the resulting transaction. Otherwise we'll retrieve
	/// a reasonable estimate from BitcoinD.
	pub fee_rate_sat_per_vb: Option<u64>,
}
/// The response `content` for the `OnchainSend` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OnchainSendResponse {
	/// The transaction ID of the broadcasted transaction.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub txid: [u8; 32],
	/// The payment ID for this on-chain payment, usable with `GetPaymentDetails`.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub payment_id: [u8; 32],
}
/// Return a BOLT11 payable invoice that can be used to request and receive a payment
/// for the given amount, if specified.
/// The inbound payment will be automatically claimed upon arrival.
/// See more:
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.receive>
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.receive_variable_amount>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt11ReceiveRequest {
	/// The amount in millisatoshi to send. If unset, a "zero-amount" or variable-amount invoice is returned.
	pub amount_msat: Option<u64>,
	/// An optional description to attach along with the invoice.
	/// Will be set in the description field of the encoded payment request.
	pub description: Option<super::types::Bolt11InvoiceDescription>,
	/// Invoice expiry time in seconds.
	pub expiry_secs: u32,
}
/// The response `content` for the `Bolt11Receive` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt11ReceiveResponse {
	/// An invoice for a payment within the Lightning Network.
	/// With the details of the invoice, the sender has all the data necessary to send a payment
	/// to the recipient.
	pub invoice: String,
	/// The hex-encoded 32-byte payment hash.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub payment_hash: [u8; 32],
	/// The hex-encoded 32-byte payment secret.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub payment_secret: [u8; 32],
}
/// Return a BOLT11 payable invoice for a given payment hash.
/// The inbound payment will NOT be automatically claimed upon arrival.
/// Instead, the payment will need to be manually claimed by calling `Bolt11ClaimForHash`
/// or manually failed by calling `Bolt11FailForHash`.
/// See more:
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.receive_for_hash>
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.receive_variable_amount_for_hash>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt11ReceiveForHashRequest {
	/// The amount in millisatoshi to receive. If unset, a "zero-amount" or variable-amount invoice is returned.
	pub amount_msat: Option<u64>,
	/// An optional description to attach along with the invoice.
	/// Will be set in the description field of the encoded payment request.
	pub description: Option<super::types::Bolt11InvoiceDescription>,
	/// Invoice expiry time in seconds.
	pub expiry_secs: u32,
	/// The hex-encoded 32-byte payment hash to use for the invoice.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub payment_hash: [u8; 32],
}
/// The response for the `Bolt11ReceiveForHash` API. Same shape as [`Bolt11ReceiveResponse`].
pub type Bolt11ReceiveForHashResponse = Bolt11ReceiveResponse;
/// Manually claim a payment for a given payment hash with the corresponding preimage.
/// This should be used to claim payments created via `Bolt11ReceiveForHash`.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.claim_for_hash>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt11ClaimForHashRequest {
	/// The hex-encoded 32-byte payment hash.
	/// If provided, it will be used to verify that the preimage matches.
	#[serde(default, with = "crate::serde_utils::opt_hex_32")]
	pub payment_hash: Option<[u8; 32]>,
	/// The amount in millisatoshi that is claimable.
	/// If not provided, skips amount verification.
	pub claimable_amount_msat: Option<u64>,
	/// The hex-encoded 32-byte payment preimage.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub preimage: [u8; 32],
}
/// The response `content` for the `Bolt11ClaimForHash` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt11ClaimForHashResponse {}
/// Manually fail a payment for a given payment hash.
/// This should be used to reject payments created via `Bolt11ReceiveForHash`.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.fail_for_hash>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt11FailForHashRequest {
	/// The hex-encoded 32-byte payment hash.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub payment_hash: [u8; 32],
}
/// The response `content` for the `Bolt11FailForHash` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt11FailForHashResponse {}
/// Return a BOLT11 payable invoice that can be used to request and receive a payment via an
/// LSPS2 just-in-time channel.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.receive_via_jit_channel>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt11ReceiveViaJitChannelRequest {
	/// The amount in millisatoshi to request.
	pub amount_msat: u64,
	/// An optional description to attach along with the invoice.
	/// Will be set in the description field of the encoded payment request.
	pub description: Option<super::types::Bolt11InvoiceDescription>,
	/// Invoice expiry time in seconds.
	pub expiry_secs: u32,
	/// Optional upper bound for the total fee an LSP may deduct when opening the JIT channel.
	pub max_total_lsp_fee_limit_msat: Option<u64>,
}
/// The response for the `Bolt11ReceiveViaJitChannel` API. Same shape as [`Bolt11ReceiveResponse`].
pub type Bolt11ReceiveViaJitChannelResponse = Bolt11ReceiveResponse;
/// Return a variable-amount BOLT11 invoice that can be used to receive a payment via an LSPS2
/// just-in-time channel.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.receive_variable_amount_via_jit_channel>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt11ReceiveVariableAmountViaJitChannelRequest {
	/// An optional description to attach along with the invoice.
	/// Will be set in the description field of the encoded payment request.
	pub description: Option<super::types::Bolt11InvoiceDescription>,
	/// Invoice expiry time in seconds.
	pub expiry_secs: u32,
	/// Optional upper bound for the proportional fee, in parts-per-million millisatoshis, that an
	/// LSP may deduct when opening the JIT channel.
	pub max_proportional_lsp_fee_limit_ppm_msat: Option<u64>,
}
/// The response for the `Bolt11ReceiveVariableAmountViaJitChannel` API. Same shape as [`Bolt11ReceiveResponse`].
pub type Bolt11ReceiveVariableAmountViaJitChannelResponse = Bolt11ReceiveResponse;
/// Send a payment for a BOLT11 invoice.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.send>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt11SendRequest {
	/// An invoice for a payment within the Lightning Network.
	pub invoice: String,
	/// Set this field when paying a so-called "zero-amount" invoice, i.e., an invoice that leaves the
	/// amount paid to be determined by the user.
	/// This operation will fail if the amount specified is less than the value required by the given invoice.
	pub amount_msat: Option<u64>,
	/// Configuration options for payment routing and pathfinding.
	pub route_parameters: Option<super::types::RouteParametersConfig>,
}
/// The response `content` for the `Bolt11Send` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt11SendResponse {
	/// An identifier used to uniquely identify a payment in hex-encoded form.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub payment_id: [u8; 32],
}
/// Returns a BOLT12 offer for the given amount, if specified.
///
/// See more:
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt12Payment.html#method.receive>
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt12Payment.html#method.receive_variable_amount>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt12ReceiveRequest {
	/// An optional description to attach along with the offer.
	/// Will be set in the description field of the encoded offer.
	pub description: String,
	/// The amount in millisatoshi to send. If unset, a "zero-amount" or variable-amount offer is returned.
	pub amount_msat: Option<u64>,
	/// Offer expiry time in seconds.
	pub expiry_secs: Option<u32>,
	/// If set, it represents the number of items requested, can only be set for fixed-amount offers.
	pub quantity: Option<u64>,
}
/// The response `content` for the `Bolt12Receive` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt12ReceiveResponse {
	/// An offer for a payment within the Lightning Network.
	/// With the details of the offer, the sender has all the data necessary to send a payment
	/// to the recipient.
	pub offer: String,
	/// The hex-encoded offer id.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub offer_id: [u8; 32],
}
/// Send a payment for a BOLT12 offer.
/// See more:
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt12Payment.html#method.send>
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt12Payment.html#method.send_using_amount>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt12SendRequest {
	/// An offer for a payment within the Lightning Network.
	pub offer: String,
	/// Set this field when paying a so-called "zero-amount" offer, i.e., an offer that leaves the
	/// amount paid to be determined by the user.
	/// This operation will fail if the amount specified is less than the value required by the given offer.
	pub amount_msat: Option<u64>,
	/// If set, it represents the number of items requested.
	pub quantity: Option<u64>,
	/// If set, it will be seen by the recipient and reflected back in the invoice.
	pub payer_note: Option<String>,
	/// Configuration options for payment routing and pathfinding.
	pub route_parameters: Option<super::types::RouteParametersConfig>,
}
/// The response `content` for the `Bolt12Send` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt12SendResponse {
	/// An identifier used to uniquely identify a payment in hex-encoded form.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub payment_id: [u8; 32],
}
/// Send a spontaneous payment, also known as "keysend", to a node.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.SpontaneousPayment.html#method.send>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpontaneousSendRequest {
	/// The amount in millisatoshis to send.
	pub amount_msat: u64,
	/// The hex-encoded public key of the node to send the payment to.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub node_id: [u8; 33],
	/// Configuration options for payment routing and pathfinding.
	pub route_parameters: Option<super::types::RouteParametersConfig>,
}
/// The response `content` for the `SpontaneousSend` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpontaneousSendResponse {
	/// An identifier used to uniquely identify a payment in hex-encoded form.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub payment_id: [u8; 32],
}
/// Creates a new outbound channel to the given remote node.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.connect_open_channel>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenChannelRequest {
	/// The hex-encoded public key of the node to open a channel with.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub node_pubkey: [u8; 33],
	/// An address which can be used to connect to a remote peer.
	/// It can be of type IPv4:port, IPv6:port, OnionV3:port or hostname:port
	pub address: String,
	/// The amount of satoshis the caller is willing to commit to the channel.
	pub channel_amount_sats: u64,
	/// The amount of satoshis to push to the remote side as part of the initial commitment state.
	pub push_to_counterparty_msat: Option<u64>,
	/// The channel configuration to be used for opening this channel. If unset, default ChannelConfig is used.
	pub channel_config: Option<super::types::ChannelConfig>,
	/// Whether the channel should be public.
	pub announce_channel: bool,
}
/// The response `content` for the `OpenChannel` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenChannelResponse {
	/// The local channel id of the created channel that user can use to refer to channel.
	pub user_channel_id: String,
}
/// Increases the channel balance by the given amount.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.splice_in>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpliceInRequest {
	/// The local `user_channel_id` of the channel.
	pub user_channel_id: String,
	/// The hex-encoded public key of the channel's counterparty node.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub counterparty_node_id: [u8; 33],
	/// The amount of sats to splice into the channel.
	pub splice_amount_sats: u64,
}
/// The response `content` for the `SpliceIn` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpliceInResponse {}
/// Decreases the channel balance by the given amount.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.splice_out>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpliceOutRequest {
	/// The local `user_channel_id` of this channel.
	pub user_channel_id: String,
	/// The hex-encoded public key of the channel's counterparty node.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub counterparty_node_id: [u8; 33],
	/// A Bitcoin on-chain address to send the spliced-out funds.
	///
	/// If not set, an address from the node's on-chain wallet will be used.
	pub address: Option<String>,
	/// The amount of sats to splice out of the channel.
	pub splice_amount_sats: u64,
}
/// The response `content` for the `SpliceOut` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpliceOutResponse {
	/// The Bitcoin on-chain address where the funds will be sent.
	pub address: String,
}
/// Update the config for a previously opened channel.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.update_channel_config>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UpdateChannelConfigRequest {
	/// The local `user_channel_id` of this channel.
	pub user_channel_id: String,
	/// The hex-encoded public key of the counterparty node to update channel config with.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub counterparty_node_id: [u8; 33],
	/// The updated channel configuration settings for a channel.
	pub channel_config: Option<super::types::ChannelConfig>,
}
/// The response `content` for the `UpdateChannelConfig` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UpdateChannelConfigResponse {}
/// Closes the channel specified by given request.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.close_channel>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloseChannelRequest {
	/// The local `user_channel_id` of this channel.
	pub user_channel_id: String,
	/// The hex-encoded public key of the node to close a channel with.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub counterparty_node_id: [u8; 33],
}
/// The response `content` for the `CloseChannel` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloseChannelResponse {}
/// Force closes the channel specified by given request.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.force_close_channel>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForceCloseChannelRequest {
	/// The local `user_channel_id` of this channel.
	pub user_channel_id: String,
	/// The hex-encoded public key of the node to close a channel with.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub counterparty_node_id: [u8; 33],
	/// The reason for force-closing.
	pub force_close_reason: Option<String>,
}
/// The response `content` for the `ForceCloseChannel` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForceCloseChannelResponse {}
/// Returns a list of known channels.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.list_channels>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListChannelsRequest {}
/// The response `content` for the `ListChannels` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListChannelsResponse {
	/// List of channels.
	pub channels: Vec<super::types::Channel>,
}
/// Returns payment details for a given payment_id.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.payment>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GetPaymentDetailsRequest {
	/// An identifier used to uniquely identify a payment in hex-encoded form.
	#[serde(with = "crate::serde_utils::hex_32")]
	pub payment_id: [u8; 32],
}
/// The response `content` for the `GetPaymentDetails` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GetPaymentDetailsResponse {
	/// Represents a payment.
	/// Will be `None` if payment doesn't exist.
	pub payment: Option<super::types::Payment>,
}
/// Retrieves list of all payments.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.list_payments>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListPaymentsRequest {
	/// `page_token` is a pagination token.
	///
	/// To query for the first page, `page_token` must not be specified.
	///
	/// For subsequent pages, use the value that was returned as `next_page_token` in the previous
	/// page's response.
	pub page_token: Option<String>,
}
/// The response `content` for the `ListPayments` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListPaymentsResponse {
	/// List of payments.
	pub payments: Vec<super::types::Payment>,
	/// `next_page_token` is a pagination token, used to retrieve the next page of results.
	/// Use this value to query for next-page of paginated operation, by specifying
	/// this value as the `page_token` in the next request.
	///
	/// If `next_page_token` is `None`, then the "last page" of results has been processed and
	/// there is no more data to be retrieved.
	///
	/// If `next_page_token` is not `None`, it does not necessarily mean that there is more data in the
	/// result set. The only way to know when you have reached the end of the result set is when
	/// `next_page_token` is `None`.
	///
	/// **Caution**: Clients must not assume a specific number of records to be present in a page for
	/// paginated response.
	pub next_page_token: Option<String>,
}
/// Retrieves list of all forwarded payments.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/enum.Event.html#variant.PaymentForwarded>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListForwardedPaymentsRequest {
	/// `page_token` is a pagination token.
	///
	/// To query for the first page, `page_token` must not be specified.
	///
	/// For subsequent pages, use the value that was returned as `next_page_token` in the previous
	/// page's response.
	pub page_token: Option<String>,
}
/// The response `content` for the `ListForwardedPayments` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListForwardedPaymentsResponse {
	/// List of forwarded payments.
	pub forwarded_payments: Vec<super::types::ForwardedPayment>,
	/// `next_page_token` is a pagination token, used to retrieve the next page of results.
	/// Use this value to query for next-page of paginated operation, by specifying
	/// this value as the `page_token` in the next request.
	///
	/// If `next_page_token` is `None`, then the "last page" of results has been processed and
	/// there is no more data to be retrieved.
	///
	/// If `next_page_token` is not `None`, it does not necessarily mean that there is more data in the
	/// result set. The only way to know when you have reached the end of the result set is when
	/// `next_page_token` is `None`.
	///
	/// **Caution**: Clients must not assume a specific number of records to be present in a page for
	/// paginated response.
	pub next_page_token: Option<String>,
}
/// Sign a message with the node's secret key.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.sign_message>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignMessageRequest {
	/// The message to sign, as raw bytes.
	#[serde(with = "crate::serde_utils::bytes_hex")]
	pub message: Vec<u8>,
}
/// The response `content` for the `SignMessage` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignMessageResponse {
	/// The signature of the message, as a zbase32-encoded string.
	pub signature: String,
}
/// Verify a signature against a message and public key.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.verify_signature>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerifySignatureRequest {
	/// The message that was signed, as raw bytes.
	#[serde(with = "crate::serde_utils::bytes_hex")]
	pub message: Vec<u8>,
	/// The signature to verify, as a zbase32-encoded string.
	pub signature: String,
	/// The hex-encoded public key of the signer.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub public_key: [u8; 33],
}
/// The response `content` for the `VerifySignature` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerifySignatureResponse {
	/// Whether the signature is valid.
	pub valid: bool,
}
/// Export the pathfinding scores used by the router.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.export_pathfinding_scores>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportPathfindingScoresRequest {}
/// The response `content` for the `ExportPathfindingScores` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportPathfindingScoresResponse {
	/// The serialized pathfinding scores data.
	#[serde(with = "crate::serde_utils::bytes_hex")]
	pub scores: Vec<u8>,
}
/// Retrieves an overview of all known balances.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.list_balances>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GetBalancesRequest {}
/// The response `content` for the `GetBalances` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GetBalancesResponse {
	/// The total balance of our on-chain wallet.
	pub total_onchain_balance_sats: u64,
	/// The currently spendable balance of our on-chain wallet.
	///
	/// This includes any sufficiently confirmed funds, minus `total_anchor_channels_reserve_sats`.
	pub spendable_onchain_balance_sats: u64,
	/// The share of our total balance that we retain as an emergency reserve to (hopefully) be
	/// able to spend the Anchor outputs when one of our channels is closed.
	pub total_anchor_channels_reserve_sats: u64,
	/// The total balance that we would be able to claim across all our Lightning channels.
	///
	/// Note this excludes balances that we are unsure if we are able to claim (e.g., as we are
	/// waiting for a preimage or for a timeout to expire). These balances will however be included
	/// as `MaybePreimageClaimableHTLC` and `MaybeTimeoutClaimableHTLC` in `lightning_balances`.
	pub total_lightning_balance_sats: u64,
	/// A detailed list of all known Lightning balances that would be claimable on channel closure.
	///
	/// Note that less than the listed amounts are spendable over lightning as further reserve
	/// restrictions apply. Please refer to `Channel::outbound_capacity_msat` and
	/// Channel::next_outbound_htlc_limit_msat as returned by `ListChannels`
	/// for a better approximation of the spendable amounts.
	pub lightning_balances: Vec<super::types::LightningBalance>,
	/// A detailed list of balances currently being swept from the Lightning to the on-chain
	/// wallet.
	///
	/// These are balances resulting from channel closures that may have been encumbered by a
	/// delay, but are now being claimed and useable once sufficiently confirmed on-chain.
	///
	/// Note that, depending on the sync status of the wallets, swept balances listed here might or
	/// might not already be accounted for in `total_onchain_balance_sats`.
	pub pending_balances_from_channel_closures: Vec<super::types::PendingSweepBalance>,
}
/// Connect to a peer on the Lightning Network.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.connect>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectPeerRequest {
	/// The hex-encoded public key of the node to connect to.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub node_pubkey: [u8; 33],
	/// An address which can be used to connect to a remote peer.
	/// It can be of type IPv4:port, IPv6:port, OnionV3:port or hostname:port
	pub address: String,
	/// Whether to persist the peer connection, i.e., whether the peer will be re-connected on
	/// restart.
	pub persist: bool,
}
/// The response `content` for the `ConnectPeer` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectPeerResponse {}
/// Disconnect from a peer and remove it from the peer store.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.disconnect>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisconnectPeerRequest {
	/// The hex-encoded public key of the node to disconnect from.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub node_pubkey: [u8; 33],
}
/// The response `content` for the `DisconnectPeer` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisconnectPeerResponse {}
/// Returns a list of peers.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.list_peers>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListPeersRequest {}
/// The response `content` for the `ListPeers` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListPeersResponse {
	/// List of peers.
	pub peers: Vec<super::types::Peer>,
}
/// Returns a list of all known short channel IDs in the network graph.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/graph/struct.NetworkGraph.html#method.list_channels>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphListChannelsRequest {}
/// The response `content` for the `GraphListChannels` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphListChannelsResponse {
	/// List of short channel IDs known to the network graph.
	pub short_channel_ids: Vec<u64>,
}
/// Returns information on a channel with the given short channel ID from the network graph.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/graph/struct.NetworkGraph.html#method.channel>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphGetChannelRequest {
	/// The short channel ID to look up.
	pub short_channel_id: u64,
}
/// The response `content` for the `GraphGetChannel` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphGetChannelResponse {
	/// The channel information.
	pub channel: Option<super::types::GraphChannel>,
}
/// Returns a list of all known node IDs in the network graph.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/graph/struct.NetworkGraph.html#method.list_nodes>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphListNodesRequest {}
/// The response `content` for the `GraphListNodes` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphListNodesResponse {
	/// List of hex-encoded node IDs known to the network graph.
	pub node_ids: Vec<String>,
}
/// Send a payment given a BIP 21 URI or BIP 353 Human-Readable Name.
///
/// This method parses the provided URI string and attempts to send the payment. If the URI
/// has an offer and/or invoice, it will try to pay the offer first followed by the invoice.
/// If they both fail, the on-chain payment will be paid.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.UnifiedPayment.html#method.send>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnifiedSendRequest {
	/// A BIP 21 URI or BIP 353 Human-Readable Name to pay.
	pub uri: String,
	/// The amount in millisatoshis to send. Required for "zero-amount" or variable-amount URIs.
	pub amount_msat: Option<u64>,
	/// Configuration options for payment routing and pathfinding.
	pub route_parameters: Option<super::types::RouteParametersConfig>,
}
/// The response `content` for the `UnifiedSend` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedSendResponse {
	/// An on-chain payment was made.
	Onchain {
		/// The transaction ID of the broadcasted transaction.
		#[serde(with = "crate::serde_utils::hex_32")]
		txid: [u8; 32],
		/// The payment ID for this on-chain payment, usable with `GetPaymentDetails`.
		#[serde(with = "crate::serde_utils::hex_32")]
		payment_id: [u8; 32],
	},
	/// A BOLT11 payment was made.
	Bolt11 {
		/// An identifier used to uniquely identify a payment in hex-encoded form.
		#[serde(with = "crate::serde_utils::hex_32")]
		payment_id: [u8; 32],
	},
	/// A BOLT12 payment was made.
	Bolt12 {
		/// An identifier used to uniquely identify a payment in hex-encoded form.
		#[serde(with = "crate::serde_utils::hex_32")]
		payment_id: [u8; 32],
	},
}
/// Returns information on a node with the given ID from the network graph.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/graph/struct.NetworkGraph.html#method.node>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphGetNodeRequest {
	/// The hex-encoded node ID to look up.
	#[serde(with = "crate::serde_utils::hex_33")]
	pub node_id: [u8; 33],
}
/// The response `content` for the `GraphGetNode` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphGetNodeResponse {
	/// The node information.
	pub node: Option<super::types::GraphNode>,
}
/// Decode a BOLT11 invoice and return its parsed fields.
/// This does not require a running node — it only parses the invoice string.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecodeInvoiceRequest {
	/// The BOLT11 invoice string to decode.
	pub invoice: String,
}
/// The response `content` for the `DecodeInvoice` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecodeInvoiceResponse {
	/// The hex-encoded public key of the destination node.
	pub destination: String,
	/// The hex-encoded 32-byte payment hash.
	pub payment_hash: String,
	/// The amount in millisatoshis, if specified in the invoice.
	pub amount_msat: Option<u64>,
	/// The creation timestamp in seconds since the UNIX epoch.
	pub timestamp: u64,
	/// The invoice expiry time in seconds.
	pub expiry: u64,
	/// The invoice description, if a direct description was provided.
	pub description: Option<String>,
	/// The hex-encoded SHA-256 hash of the description, if a description hash was used.
	pub description_hash: Option<String>,
	/// The fallback on-chain address, if any.
	pub fallback_address: Option<String>,
	/// The minimum final CLTV expiry delta.
	pub min_final_cltv_expiry_delta: u64,
	/// The hex-encoded 32-byte payment secret.
	pub payment_secret: String,
	/// Route hints for finding a path to the payee.
	pub route_hints: Vec<super::types::Bolt11RouteHint>,
	/// Feature bits advertised in the invoice, keyed by bit number.
	pub features: ::std::collections::HashMap<u32, super::types::Bolt11Feature>,
	/// The currency or network (e.g., "bitcoin", "testnet", "signet", "regtest").
	pub currency: String,
	/// The payment metadata, hex-encoded. Only present if the invoice includes payment metadata.
	pub payment_metadata: Option<String>,
	/// Whether the invoice has expired.
	pub is_expired: bool,
}
/// Decode a BOLT12 offer and return its parsed fields.
/// This does not require a running node — it only parses the offer string.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecodeOfferRequest {
	/// The BOLT12 offer string to decode.
	pub offer: String,
}
/// The response `content` for the `DecodeOffer` API, when HttpStatusCode is OK (200).
/// When HttpStatusCode is not OK (non-200), the response `content` contains a serialized `ErrorResponse`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecodeOfferResponse {
	/// The hex-encoded offer ID.
	pub offer_id: String,
	/// The description of the offer, if any.
	pub description: Option<String>,
	/// The issuer of the offer, if any.
	pub issuer: Option<String>,
	/// The amount, if specified.
	pub amount: Option<super::types::OfferAmount>,
	/// The hex-encoded public key used by the issuer to sign invoices, if any.
	pub issuer_signing_pubkey: Option<String>,
	/// The absolute expiry time in seconds since the UNIX epoch, if any.
	pub absolute_expiry: Option<u64>,
	/// The supported quantity of items.
	pub quantity: Option<super::types::OfferQuantity>,
	/// Blinded paths to the offer recipient.
	pub paths: Vec<super::types::BlindedPath>,
	/// Feature bits advertised in the offer, keyed by bit number.
	pub features: ::std::collections::HashMap<u32, super::types::Bolt11Feature>,
	/// Supported blockchain networks (e.g., "bitcoin", "testnet", "signet", "regtest").
	pub chains: Vec<String>,
	/// The metadata, hex-encoded, if any.
	pub metadata: Option<String>,
	/// Whether the offer has expired.
	pub is_expired: bool,
}
