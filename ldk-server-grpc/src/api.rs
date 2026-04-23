// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

/// Retrieve the latest node info like `node_id`, `current_best_block` etc.
/// See more:
/// - <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.node_id>
/// - <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.status>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetNodeInfoRequest {}
/// The response for the `GetNodeInfo` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetNodeInfoResponse {
	/// The hex-encoded `node-id` or public key for our own lightning node.
	#[prost(string, tag = "1")]
	pub node_id: ::prost::alloc::string::String,
	/// The best block to which our Lightning wallet is currently synced.
	///
	/// Should be always set, will never be `None`.
	#[prost(message, optional, tag = "3")]
	pub current_best_block: ::core::option::Option<super::types::BestBlock>,
	/// The timestamp, in seconds since start of the UNIX epoch, when we last successfully synced our Lightning wallet to
	/// the chain tip.
	///
	/// Will be `None` if the wallet hasn't been synced yet.
	#[prost(uint64, optional, tag = "4")]
	pub latest_lightning_wallet_sync_timestamp: ::core::option::Option<u64>,
	/// The timestamp, in seconds since start of the UNIX epoch, when we last successfully synced our on-chain
	/// wallet to the chain tip.
	///
	/// Will be `None` if the wallet hasn’t been synced since the node was initialized.
	#[prost(uint64, optional, tag = "5")]
	pub latest_onchain_wallet_sync_timestamp: ::core::option::Option<u64>,
	/// The timestamp, in seconds since start of the UNIX epoch, when we last successfully update our fee rate cache.
	///
	/// Will be `None` if the cache hasn’t been updated since the node was initialized.
	#[prost(uint64, optional, tag = "6")]
	pub latest_fee_rate_cache_update_timestamp: ::core::option::Option<u64>,
	/// The timestamp, in seconds since start of the UNIX epoch, when the last rapid gossip sync (RGS) snapshot we
	/// successfully applied was generated.
	///
	/// Will be `None` if RGS isn’t configured or the snapshot hasn’t been updated since the node was initialized.
	#[prost(uint64, optional, tag = "7")]
	pub latest_rgs_snapshot_timestamp: ::core::option::Option<u64>,
	/// The timestamp, in seconds since start of the UNIX epoch, when we last broadcasted a node announcement.
	///
	/// Will be `None` if we have no public channels or we haven’t broadcasted since the node was initialized.
	#[prost(uint64, optional, tag = "8")]
	pub latest_node_announcement_broadcast_timestamp: ::core::option::Option<u64>,
	/// The addresses the node is currently listening on for incoming connections.
	///
	/// Will be empty if the node is not listening on any addresses.
	#[prost(string, repeated, tag = "9")]
	pub listening_addresses: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
	/// The addresses the node announces to the network.
	///
	/// Will be empty if no announcement addresses are configured.
	#[prost(string, repeated, tag = "10")]
	pub announcement_addresses: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
	/// The node alias, if configured.
	///
	/// Will be `None` if no alias is configured.
	#[prost(string, optional, tag = "11")]
	pub node_alias: ::core::option::Option<::prost::alloc::string::String>,
	/// The node URIs that can be used to connect to this node, in the format `node_id@address`.
	///
	/// These are constructed from the announcement addresses and the node's public key.
	/// Will be empty if no announcement addresses are configured.
	#[prost(string, repeated, tag = "12")]
	pub node_uris: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// Retrieve a new on-chain funding address.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.OnchainPayment.html#method.new_address>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OnchainReceiveRequest {}
/// The response for the `OnchainReceive` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OnchainReceiveResponse {
	/// A Bitcoin on-chain address.
	#[prost(string, tag = "1")]
	pub address: ::prost::alloc::string::String,
}
/// Send an on-chain payment to the given address.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OnchainSendRequest {
	/// The address to send coins to.
	#[prost(string, tag = "1")]
	pub address: ::prost::alloc::string::String,
	/// The amount in satoshis to send.
	/// While sending the specified amount, we will respect any on-chain reserve we need to keep,
	/// i.e., won't allow to cut into `total_anchor_channels_reserve_sats`.
	/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.OnchainPayment.html#method.send_to_address>
	#[prost(uint64, optional, tag = "2")]
	pub amount_sats: ::core::option::Option<u64>,
	/// If set, the amount_sats field should be unset.
	/// It indicates that node will send full balance to the specified address.
	///
	/// Please note that when send_all is used this operation will **not** retain any on-chain reserves,
	/// which might be potentially dangerous if you have open Anchor channels for which you can't trust
	/// the counterparty to spend the Anchor output after channel closure.
	/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.OnchainPayment.html#method.send_all_to_address>
	#[prost(bool, optional, tag = "3")]
	pub send_all: ::core::option::Option<bool>,
	/// If `fee_rate_sat_per_vb` is set it will be used on the resulting transaction. Otherwise we'll retrieve
	/// a reasonable estimate from BitcoinD.
	#[prost(uint64, optional, tag = "4")]
	pub fee_rate_sat_per_vb: ::core::option::Option<u64>,
}
/// The response for the `OnchainSend` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OnchainSendResponse {
	/// The transaction ID of the broadcasted transaction.
	#[prost(string, tag = "1")]
	pub txid: ::prost::alloc::string::String,
}
/// Return a BOLT11 payable invoice that can be used to request and receive a payment
/// for the given amount, if specified.
/// The inbound payment will be automatically claimed upon arrival.
/// See more:
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.receive>
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.receive_variable_amount>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11ReceiveRequest {
	/// The amount in millisatoshi to send. If unset, a "zero-amount" or variable-amount invoice is returned.
	#[prost(uint64, optional, tag = "1")]
	pub amount_msat: ::core::option::Option<u64>,
	/// An optional description to attach along with the invoice.
	/// Will be set in the description field of the encoded payment request.
	#[prost(message, optional, tag = "2")]
	pub description: ::core::option::Option<super::types::Bolt11InvoiceDescription>,
	/// Invoice expiry time in seconds.
	#[prost(uint32, tag = "3")]
	pub expiry_secs: u32,
}
/// The response for the `Bolt11Receive` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11ReceiveResponse {
	/// An invoice for a payment within the Lightning Network.
	/// With the details of the invoice, the sender has all the data necessary to send a payment
	/// to the recipient.
	#[prost(string, tag = "1")]
	pub invoice: ::prost::alloc::string::String,
	/// The hex-encoded 32-byte payment hash.
	#[prost(string, tag = "2")]
	pub payment_hash: ::prost::alloc::string::String,
	/// The hex-encoded 32-byte payment secret.
	#[prost(string, tag = "3")]
	pub payment_secret: ::prost::alloc::string::String,
}
/// Return a BOLT11 payable invoice for a given payment hash.
/// The inbound payment will NOT be automatically claimed upon arrival.
/// Instead, the payment will need to be manually claimed by calling `Bolt11ClaimForHash`
/// or manually failed by calling `Bolt11FailForHash`.
/// See more:
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.receive_for_hash>
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.receive_variable_amount_for_hash>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11ReceiveForHashRequest {
	/// The amount in millisatoshi to receive. If unset, a "zero-amount" or variable-amount invoice is returned.
	#[prost(uint64, optional, tag = "1")]
	pub amount_msat: ::core::option::Option<u64>,
	/// An optional description to attach along with the invoice.
	/// Will be set in the description field of the encoded payment request.
	#[prost(message, optional, tag = "2")]
	pub description: ::core::option::Option<super::types::Bolt11InvoiceDescription>,
	/// Invoice expiry time in seconds.
	#[prost(uint32, tag = "3")]
	pub expiry_secs: u32,
	/// The hex-encoded 32-byte payment hash to use for the invoice.
	#[prost(string, tag = "4")]
	pub payment_hash: ::prost::alloc::string::String,
}
/// The response for the `Bolt11ReceiveForHash` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11ReceiveForHashResponse {
	/// An invoice for a payment within the Lightning Network.
	/// With the details of the invoice, the sender has all the data necessary to send a payment
	/// to the recipient.
	#[prost(string, tag = "1")]
	pub invoice: ::prost::alloc::string::String,
}
/// Manually claim a payment for a given payment hash with the corresponding preimage.
/// This should be used to claim payments created via `Bolt11ReceiveForHash`.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.claim_for_hash>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11ClaimForHashRequest {
	/// The hex-encoded 32-byte payment hash.
	/// If provided, it will be used to verify that the preimage matches.
	#[prost(string, optional, tag = "1")]
	pub payment_hash: ::core::option::Option<::prost::alloc::string::String>,
	/// The amount in millisatoshi that is claimable.
	/// If not provided, skips amount verification.
	#[prost(uint64, optional, tag = "2")]
	pub claimable_amount_msat: ::core::option::Option<u64>,
	/// The hex-encoded 32-byte payment preimage.
	#[prost(string, tag = "3")]
	pub preimage: ::prost::alloc::string::String,
}
/// The response for the `Bolt11ClaimForHash` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11ClaimForHashResponse {}
/// Manually fail a payment for a given payment hash.
/// This should be used to reject payments created via `Bolt11ReceiveForHash`.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.fail_for_hash>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11FailForHashRequest {
	/// The hex-encoded 32-byte payment hash.
	#[prost(string, tag = "1")]
	pub payment_hash: ::prost::alloc::string::String,
}
/// The response for the `Bolt11FailForHash` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11FailForHashResponse {}
/// Return a BOLT11 payable invoice that can be used to request and receive a payment via an
/// LSPS2 just-in-time channel.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.receive_via_jit_channel>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11ReceiveViaJitChannelRequest {
	/// The amount in millisatoshi to request.
	#[prost(uint64, tag = "1")]
	pub amount_msat: u64,
	/// An optional description to attach along with the invoice.
	/// Will be set in the description field of the encoded payment request.
	#[prost(message, optional, tag = "2")]
	pub description: ::core::option::Option<super::types::Bolt11InvoiceDescription>,
	/// Invoice expiry time in seconds.
	#[prost(uint32, tag = "3")]
	pub expiry_secs: u32,
	/// Optional upper bound for the total fee an LSP may deduct when opening the JIT channel.
	#[prost(uint64, optional, tag = "4")]
	pub max_total_lsp_fee_limit_msat: ::core::option::Option<u64>,
}
/// The response for the `Bolt11ReceiveViaJitChannel` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11ReceiveViaJitChannelResponse {
	/// An invoice for a payment within the Lightning Network.
	#[prost(string, tag = "1")]
	pub invoice: ::prost::alloc::string::String,
}
/// Return a variable-amount BOLT11 invoice that can be used to receive a payment via an LSPS2
/// just-in-time channel.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.receive_variable_amount_via_jit_channel>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11ReceiveVariableAmountViaJitChannelRequest {
	/// An optional description to attach along with the invoice.
	/// Will be set in the description field of the encoded payment request.
	#[prost(message, optional, tag = "1")]
	pub description: ::core::option::Option<super::types::Bolt11InvoiceDescription>,
	/// Invoice expiry time in seconds.
	#[prost(uint32, tag = "2")]
	pub expiry_secs: u32,
	/// Optional upper bound for the proportional fee, in parts-per-million millisatoshis, that an
	/// LSP may deduct when opening the JIT channel.
	#[prost(uint64, optional, tag = "3")]
	pub max_proportional_lsp_fee_limit_ppm_msat: ::core::option::Option<u64>,
}
/// The response for the `Bolt11ReceiveVariableAmountViaJitChannel` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11ReceiveVariableAmountViaJitChannelResponse {
	/// An invoice for a payment within the Lightning Network.
	#[prost(string, tag = "1")]
	pub invoice: ::prost::alloc::string::String,
}
/// Send a payment for a BOLT11 invoice.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt11Payment.html#method.send>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11SendRequest {
	/// An invoice for a payment within the Lightning Network.
	#[prost(string, tag = "1")]
	pub invoice: ::prost::alloc::string::String,
	/// Set this field when paying a so-called "zero-amount" invoice, i.e., an invoice that leaves the
	/// amount paid to be determined by the user.
	/// This operation will fail if the amount specified is less than the value required by the given invoice.
	#[prost(uint64, optional, tag = "2")]
	pub amount_msat: ::core::option::Option<u64>,
	/// Configuration options for payment routing and pathfinding.
	#[prost(message, optional, tag = "3")]
	pub route_parameters: ::core::option::Option<super::types::RouteParametersConfig>,
}
/// The response for the `Bolt11Send` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt11SendResponse {
	/// An identifier used to uniquely identify a payment in hex-encoded form.
	#[prost(string, tag = "1")]
	pub payment_id: ::prost::alloc::string::String,
}
/// Returns a BOLT12 offer for the given amount, if specified.
///
/// See more:
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt12Payment.html#method.receive>
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt12Payment.html#method.receive_variable_amount>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt12ReceiveRequest {
	/// An optional description to attach along with the offer.
	/// Will be set in the description field of the encoded offer.
	#[prost(string, tag = "1")]
	pub description: ::prost::alloc::string::String,
	/// The amount in millisatoshi to send. If unset, a "zero-amount" or variable-amount offer is returned.
	#[prost(uint64, optional, tag = "2")]
	pub amount_msat: ::core::option::Option<u64>,
	/// Offer expiry time in seconds.
	#[prost(uint32, optional, tag = "3")]
	pub expiry_secs: ::core::option::Option<u32>,
	/// If set, it represents the number of items requested, can only be set for fixed-amount offers.
	#[prost(uint64, optional, tag = "4")]
	pub quantity: ::core::option::Option<u64>,
}
/// The response for the `Bolt12Receive` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt12ReceiveResponse {
	/// An offer for a payment within the Lightning Network.
	/// With the details of the offer, the sender has all the data necessary to send a payment
	/// to the recipient.
	#[prost(string, tag = "1")]
	pub offer: ::prost::alloc::string::String,
	/// The hex-encoded offer id.
	#[prost(string, tag = "2")]
	pub offer_id: ::prost::alloc::string::String,
}
/// Send a payment for a BOLT12 offer.
/// See more:
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt12Payment.html#method.send>
/// - <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.Bolt12Payment.html#method.send_using_amount>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt12SendRequest {
	/// An offer for a payment within the Lightning Network.
	#[prost(string, tag = "1")]
	pub offer: ::prost::alloc::string::String,
	/// Set this field when paying a so-called "zero-amount" offer, i.e., an offer that leaves the
	/// amount paid to be determined by the user.
	/// This operation will fail if the amount specified is less than the value required by the given offer.
	#[prost(uint64, optional, tag = "2")]
	pub amount_msat: ::core::option::Option<u64>,
	/// If set, it represents the number of items requested.
	#[prost(uint64, optional, tag = "3")]
	pub quantity: ::core::option::Option<u64>,
	/// If set, it will be seen by the recipient and reflected back in the invoice.
	#[prost(string, optional, tag = "4")]
	pub payer_note: ::core::option::Option<::prost::alloc::string::String>,
	/// Configuration options for payment routing and pathfinding.
	#[prost(message, optional, tag = "5")]
	pub route_parameters: ::core::option::Option<super::types::RouteParametersConfig>,
}
/// The response for the `Bolt12Send` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bolt12SendResponse {
	/// An identifier used to uniquely identify a payment in hex-encoded form.
	#[prost(string, tag = "1")]
	pub payment_id: ::prost::alloc::string::String,
}
/// Send a spontaneous payment, also known as "keysend", to a node.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.SpontaneousPayment.html#method.send>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpontaneousSendRequest {
	/// The amount in millisatoshis to send.
	#[prost(uint64, tag = "1")]
	pub amount_msat: u64,
	/// The hex-encoded public key of the node to send the payment to.
	#[prost(string, tag = "2")]
	pub node_id: ::prost::alloc::string::String,
	/// Configuration options for payment routing and pathfinding.
	#[prost(message, optional, tag = "3")]
	pub route_parameters: ::core::option::Option<super::types::RouteParametersConfig>,
}
/// The response for the `SpontaneousSend` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpontaneousSendResponse {
	/// An identifier used to uniquely identify a payment in hex-encoded form.
	#[prost(string, tag = "1")]
	pub payment_id: ::prost::alloc::string::String,
}
/// Creates a new outbound channel to the given remote node.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.connect_open_channel>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OpenChannelRequest {
	/// The hex-encoded public key of the node to open a channel with.
	#[prost(string, tag = "1")]
	pub node_pubkey: ::prost::alloc::string::String,
	/// An address which can be used to connect to a remote peer.
	/// It can be of type IPv4:port, IPv6:port, OnionV3:port or hostname:port
	#[prost(string, tag = "2")]
	pub address: ::prost::alloc::string::String,
	/// The amount of satoshis the caller is willing to commit to the channel.
	#[prost(uint64, tag = "3")]
	pub channel_amount_sats: u64,
	/// The amount of satoshis to push to the remote side as part of the initial commitment state.
	#[prost(uint64, optional, tag = "4")]
	pub push_to_counterparty_msat: ::core::option::Option<u64>,
	/// The channel configuration to be used for opening this channel. If unset, default ChannelConfig is used.
	#[prost(message, optional, tag = "5")]
	pub channel_config: ::core::option::Option<super::types::ChannelConfig>,
	/// Whether the channel should be public.
	#[prost(bool, tag = "6")]
	pub announce_channel: bool,
	/// Allow the counterparty to spend all its channel balance. This cannot be set together with `announce_channel`.
	#[prost(bool, tag = "7")]
	pub disable_counterparty_reserve: bool,
}
/// The response for the `OpenChannel` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OpenChannelResponse {
	/// The local channel id of the created channel that user can use to refer to channel.
	#[prost(string, tag = "1")]
	pub user_channel_id: ::prost::alloc::string::String,
}
/// Increases the channel balance by the given amount.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.splice_in>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpliceInRequest {
	/// The local `user_channel_id` of the channel.
	#[prost(string, tag = "1")]
	pub user_channel_id: ::prost::alloc::string::String,
	/// The hex-encoded public key of the channel's counterparty node.
	#[prost(string, tag = "2")]
	pub counterparty_node_id: ::prost::alloc::string::String,
	/// The amount of sats to splice into the channel.
	#[prost(uint64, tag = "3")]
	pub splice_amount_sats: u64,
}
/// The response for the `SpliceIn` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpliceInResponse {}
/// Decreases the channel balance by the given amount.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.splice_out>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpliceOutRequest {
	/// The local `user_channel_id` of this channel.
	#[prost(string, tag = "1")]
	pub user_channel_id: ::prost::alloc::string::String,
	/// The hex-encoded public key of the channel's counterparty node.
	#[prost(string, tag = "2")]
	pub counterparty_node_id: ::prost::alloc::string::String,
	/// A Bitcoin on-chain address to send the spliced-out funds.
	///
	/// If not set, an address from the node's on-chain wallet will be used.
	#[prost(string, optional, tag = "3")]
	pub address: ::core::option::Option<::prost::alloc::string::String>,
	/// The amount of sats to splice out of the channel.
	#[prost(uint64, tag = "4")]
	pub splice_amount_sats: u64,
}
/// The response for the `SpliceOut` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpliceOutResponse {
	/// The Bitcoin on-chain address where the funds will be sent.
	#[prost(string, tag = "1")]
	pub address: ::prost::alloc::string::String,
}
/// Update the config for a previously opened channel.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.update_channel_config>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateChannelConfigRequest {
	/// The local `user_channel_id` of this channel.
	#[prost(string, tag = "1")]
	pub user_channel_id: ::prost::alloc::string::String,
	/// The hex-encoded public key of the counterparty node to update channel config with.
	#[prost(string, tag = "2")]
	pub counterparty_node_id: ::prost::alloc::string::String,
	/// The updated channel configuration settings for a channel.
	#[prost(message, optional, tag = "3")]
	pub channel_config: ::core::option::Option<super::types::ChannelConfig>,
}
/// The response for the `UpdateChannelConfig` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateChannelConfigResponse {}
/// Closes the channel specified by given request.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.close_channel>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CloseChannelRequest {
	/// The local `user_channel_id` of this channel.
	#[prost(string, tag = "1")]
	pub user_channel_id: ::prost::alloc::string::String,
	/// The hex-encoded public key of the node to close a channel with.
	#[prost(string, tag = "2")]
	pub counterparty_node_id: ::prost::alloc::string::String,
}
/// The response for the `CloseChannel` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CloseChannelResponse {}
/// Force closes the channel specified by given request.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.force_close_channel>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ForceCloseChannelRequest {
	/// The local `user_channel_id` of this channel.
	#[prost(string, tag = "1")]
	pub user_channel_id: ::prost::alloc::string::String,
	/// The hex-encoded public key of the node to close a channel with.
	#[prost(string, tag = "2")]
	pub counterparty_node_id: ::prost::alloc::string::String,
	/// The reason for force-closing.
	#[prost(string, optional, tag = "3")]
	pub force_close_reason: ::core::option::Option<::prost::alloc::string::String>,
}
/// The response for the `ForceCloseChannel` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ForceCloseChannelResponse {}
/// Returns a list of known channels.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.list_channels>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListChannelsRequest {}
/// The response for the `ListChannels` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListChannelsResponse {
	/// List of channels.
	#[prost(message, repeated, tag = "1")]
	pub channels: ::prost::alloc::vec::Vec<super::types::Channel>,
}
/// Returns payment details for a given payment_id.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.payment>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPaymentDetailsRequest {
	/// An identifier used to uniquely identify a payment in hex-encoded form.
	#[prost(string, tag = "1")]
	pub payment_id: ::prost::alloc::string::String,
}
/// The response for the `GetPaymentDetails` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPaymentDetailsResponse {
	/// Represents a payment.
	/// Will be `None` if payment doesn't exist.
	#[prost(message, optional, tag = "1")]
	pub payment: ::core::option::Option<super::types::Payment>,
}
/// Retrieves list of all payments.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.list_payments>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListPaymentsRequest {
	/// `page_token` is a pagination token.
	///
	/// To query for the first page, `page_token` must not be specified.
	///
	/// For subsequent pages, use the value that was returned as `next_page_token` in the previous
	/// page's response.
	#[prost(message, optional, tag = "1")]
	pub page_token: ::core::option::Option<super::types::PageToken>,
}
/// The response for the `ListPayments` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListPaymentsResponse {
	/// List of payments.
	#[prost(message, repeated, tag = "1")]
	pub payments: ::prost::alloc::vec::Vec<super::types::Payment>,
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
	#[prost(message, optional, tag = "2")]
	pub next_page_token: ::core::option::Option<super::types::PageToken>,
}
/// Retrieves list of all forwarded payments.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/enum.Event.html#variant.PaymentForwarded>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListForwardedPaymentsRequest {
	/// `page_token` is a pagination token.
	///
	/// To query for the first page, `page_token` must not be specified.
	///
	/// For subsequent pages, use the value that was returned as `next_page_token` in the previous
	/// page's response.
	#[prost(message, optional, tag = "1")]
	pub page_token: ::core::option::Option<super::types::PageToken>,
}
/// The response for the `ListForwardedPayments` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListForwardedPaymentsResponse {
	/// List of forwarded payments.
	#[prost(message, repeated, tag = "1")]
	pub forwarded_payments: ::prost::alloc::vec::Vec<super::types::ForwardedPayment>,
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
	#[prost(message, optional, tag = "2")]
	pub next_page_token: ::core::option::Option<super::types::PageToken>,
}
/// Sign a message with the node's secret key.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.sign_message>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SignMessageRequest {
	/// The message to sign, as raw bytes.
	#[prost(bytes = "bytes", tag = "1")]
	pub message: ::prost::bytes::Bytes,
}
/// The response for the `SignMessage` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SignMessageResponse {
	/// The signature of the message, as a zbase32-encoded string.
	#[prost(string, tag = "1")]
	pub signature: ::prost::alloc::string::String,
}
/// Verify a signature against a message and public key.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.verify_signature>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifySignatureRequest {
	/// The message that was signed, as raw bytes.
	#[prost(bytes = "bytes", tag = "1")]
	pub message: ::prost::bytes::Bytes,
	/// The signature to verify, as a zbase32-encoded string.
	#[prost(string, tag = "2")]
	pub signature: ::prost::alloc::string::String,
	/// The hex-encoded public key of the signer.
	#[prost(string, tag = "3")]
	pub public_key: ::prost::alloc::string::String,
}
/// The response for the `VerifySignature` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifySignatureResponse {
	/// Whether the signature is valid.
	#[prost(bool, tag = "1")]
	pub valid: bool,
}
/// Export the pathfinding scores used by the router.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.export_pathfinding_scores>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportPathfindingScoresRequest {}
/// The response for the `ExportPathfindingScores` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportPathfindingScoresResponse {
	/// The serialized pathfinding scores data.
	#[prost(bytes = "bytes", tag = "1")]
	pub scores: ::prost::bytes::Bytes,
}
/// Retrieves an overview of all known balances.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.list_balances>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBalancesRequest {}
/// The response for the `GetBalances` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBalancesResponse {
	/// The total balance of our on-chain wallet.
	#[prost(uint64, tag = "1")]
	pub total_onchain_balance_sats: u64,
	/// The currently spendable balance of our on-chain wallet.
	///
	/// This includes any sufficiently confirmed funds, minus `total_anchor_channels_reserve_sats`.
	#[prost(uint64, tag = "2")]
	pub spendable_onchain_balance_sats: u64,
	/// The share of our total balance that we retain as an emergency reserve to (hopefully) be
	/// able to spend the Anchor outputs when one of our channels is closed.
	#[prost(uint64, tag = "3")]
	pub total_anchor_channels_reserve_sats: u64,
	/// The total balance that we would be able to claim across all our Lightning channels.
	///
	/// Note this excludes balances that we are unsure if we are able to claim (e.g., as we are
	/// waiting for a preimage or for a timeout to expire). These balances will however be included
	/// as `MaybePreimageClaimableHTLC` and `MaybeTimeoutClaimableHTLC` in `lightning_balances`.
	#[prost(uint64, tag = "4")]
	pub total_lightning_balance_sats: u64,
	/// A detailed list of all known Lightning balances that would be claimable on channel closure.
	///
	/// Note that less than the listed amounts are spendable over lightning as further reserve
	/// restrictions apply. Please refer to `Channel::outbound_capacity_msat` and
	/// Channel::next_outbound_htlc_limit_msat as returned by `ListChannels`
	/// for a better approximation of the spendable amounts.
	#[prost(message, repeated, tag = "5")]
	pub lightning_balances: ::prost::alloc::vec::Vec<super::types::LightningBalance>,
	/// A detailed list of balances currently being swept from the Lightning to the on-chain
	/// wallet.
	///
	/// These are balances resulting from channel closures that may have been encumbered by a
	/// delay, but are now being claimed and useable once sufficiently confirmed on-chain.
	///
	/// Note that, depending on the sync status of the wallets, swept balances listed here might or
	/// might not already be accounted for in `total_onchain_balance_sats`.
	#[prost(message, repeated, tag = "6")]
	pub pending_balances_from_channel_closures:
		::prost::alloc::vec::Vec<super::types::PendingSweepBalance>,
}
/// Connect to a peer on the Lightning Network.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.connect>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConnectPeerRequest {
	/// The hex-encoded public key of the node to connect to.
	#[prost(string, tag = "1")]
	pub node_pubkey: ::prost::alloc::string::String,
	/// An address which can be used to connect to a remote peer.
	/// It can be of type IPv4:port, IPv6:port, OnionV3:port or hostname:port
	#[prost(string, tag = "2")]
	pub address: ::prost::alloc::string::String,
	/// Whether to persist the peer connection, i.e., whether the peer will be re-connected on
	/// restart.
	#[prost(bool, tag = "3")]
	pub persist: bool,
}
/// The response for the `ConnectPeer` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConnectPeerResponse {}
/// Disconnect from a peer and remove it from the peer store.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.disconnect>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DisconnectPeerRequest {
	/// The hex-encoded public key of the node to disconnect from.
	#[prost(string, tag = "1")]
	pub node_pubkey: ::prost::alloc::string::String,
}
/// The response for the `DisconnectPeer` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DisconnectPeerResponse {}
/// Returns a list of peers.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/struct.Node.html#method.list_peers>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListPeersRequest {}
/// The response for the `ListPeers` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListPeersResponse {
	/// List of peers.
	#[prost(message, repeated, tag = "1")]
	pub peers: ::prost::alloc::vec::Vec<super::types::Peer>,
}
/// Returns a list of all known short channel IDs in the network graph.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/graph/struct.NetworkGraph.html#method.list_channels>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GraphListChannelsRequest {}
/// The response for the `GraphListChannels` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GraphListChannelsResponse {
	/// List of short channel IDs known to the network graph.
	#[prost(uint64, repeated, tag = "1")]
	pub short_channel_ids: ::prost::alloc::vec::Vec<u64>,
}
/// Returns information on a channel with the given short channel ID from the network graph.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/graph/struct.NetworkGraph.html#method.channel>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GraphGetChannelRequest {
	/// The short channel ID to look up.
	#[prost(uint64, tag = "1")]
	pub short_channel_id: u64,
}
/// The response for the `GraphGetChannel` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GraphGetChannelResponse {
	/// The channel information.
	#[prost(message, optional, tag = "1")]
	pub channel: ::core::option::Option<super::types::GraphChannel>,
}
/// Returns a list of all known node IDs in the network graph.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/graph/struct.NetworkGraph.html#method.list_nodes>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GraphListNodesRequest {}
/// The response for the `GraphListNodes` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GraphListNodesResponse {
	/// List of hex-encoded node IDs known to the network graph.
	#[prost(string, repeated, tag = "1")]
	pub node_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// Send a payment given a BIP 21 URI or BIP 353 Human-Readable Name.
///
/// This method parses the provided URI string and attempts to send the payment. If the URI
/// has an offer and/or invoice, it will try to pay the offer first followed by the invoice.
/// If they both fail, the on-chain payment will be paid.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/payment/struct.UnifiedPayment.html#method.send>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UnifiedSendRequest {
	/// A BIP 21 URI or BIP 353 Human-Readable Name to pay.
	#[prost(string, tag = "1")]
	pub uri: ::prost::alloc::string::String,
	/// The amount in millisatoshis to send. Required for "zero-amount" or variable-amount URIs.
	#[prost(uint64, optional, tag = "2")]
	pub amount_msat: ::core::option::Option<u64>,
	/// Configuration options for payment routing and pathfinding.
	#[prost(message, optional, tag = "3")]
	pub route_parameters: ::core::option::Option<super::types::RouteParametersConfig>,
}
/// The response for the `UnifiedSend` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UnifiedSendResponse {
	#[prost(oneof = "unified_send_response::PaymentResult", tags = "1, 2, 3")]
	#[cfg_attr(feature = "serde", serde(flatten))]
	pub payment_result: ::core::option::Option<unified_send_response::PaymentResult>,
}
/// Nested message and enum types in `UnifiedSendResponse`.
pub mod unified_send_response {
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
	#[allow(clippy::derive_partial_eq_without_eq)]
	#[derive(Clone, PartialEq, ::prost::Oneof)]
	pub enum PaymentResult {
		/// An on-chain payment was made. Contains the transaction ID.
		#[prost(string, tag = "1")]
		Txid(::prost::alloc::string::String),
		/// A BOLT11 payment was made. Contains the payment ID in hex-encoded form.
		#[prost(string, tag = "2")]
		Bolt11PaymentId(::prost::alloc::string::String),
		/// A BOLT12 payment was made. Contains the payment ID in hex-encoded form.
		#[prost(string, tag = "3")]
		Bolt12PaymentId(::prost::alloc::string::String),
	}
}
/// Returns information on a node with the given ID from the network graph.
/// See more: <https://docs.rs/ldk-node/latest/ldk_node/graph/struct.NetworkGraph.html#method.node>
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GraphGetNodeRequest {
	/// The hex-encoded node ID to look up.
	#[prost(string, tag = "1")]
	pub node_id: ::prost::alloc::string::String,
}
/// The response for the `GraphGetNode` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GraphGetNodeResponse {
	/// The node information.
	#[prost(message, optional, tag = "1")]
	pub node: ::core::option::Option<super::types::GraphNode>,
}
/// Decode a BOLT11 invoice and return its parsed fields.
/// This does not require a running node — it only parses the invoice string.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DecodeInvoiceRequest {
	/// The BOLT11 invoice string to decode.
	#[prost(string, tag = "1")]
	pub invoice: ::prost::alloc::string::String,
}
/// The response for the `DecodeInvoice` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DecodeInvoiceResponse {
	/// The hex-encoded public key of the destination node.
	#[prost(string, tag = "1")]
	pub destination: ::prost::alloc::string::String,
	/// The hex-encoded 32-byte payment hash.
	#[prost(string, tag = "2")]
	pub payment_hash: ::prost::alloc::string::String,
	/// The amount in millisatoshis, if specified in the invoice.
	#[prost(uint64, optional, tag = "3")]
	pub amount_msat: ::core::option::Option<u64>,
	/// The creation timestamp in seconds since the UNIX epoch.
	#[prost(uint64, tag = "4")]
	pub timestamp: u64,
	/// The invoice expiry time in seconds.
	#[prost(uint64, tag = "5")]
	pub expiry: u64,
	/// The invoice description, if a direct description was provided.
	#[prost(string, optional, tag = "6")]
	pub description: ::core::option::Option<::prost::alloc::string::String>,
	/// The hex-encoded SHA-256 hash of the description, if a description hash was used.
	#[prost(string, optional, tag = "14")]
	pub description_hash: ::core::option::Option<::prost::alloc::string::String>,
	/// The fallback on-chain address, if any.
	#[prost(string, optional, tag = "7")]
	pub fallback_address: ::core::option::Option<::prost::alloc::string::String>,
	/// The minimum final CLTV expiry delta.
	#[prost(uint64, tag = "8")]
	pub min_final_cltv_expiry_delta: u64,
	/// The hex-encoded 32-byte payment secret.
	#[prost(string, tag = "9")]
	pub payment_secret: ::prost::alloc::string::String,
	/// Route hints for finding a path to the payee.
	#[prost(message, repeated, tag = "10")]
	pub route_hints: ::prost::alloc::vec::Vec<super::types::Bolt11RouteHint>,
	/// Feature bits advertised in the invoice, keyed by bit number.
	#[prost(map = "uint32, message", tag = "11")]
	pub features: ::std::collections::HashMap<u32, super::types::Bolt11Feature>,
	/// The currency or network (e.g., "bitcoin", "testnet", "signet", "regtest").
	#[prost(string, tag = "12")]
	pub currency: ::prost::alloc::string::String,
	/// The payment metadata, hex-encoded. Only present if the invoice includes payment metadata.
	#[prost(string, optional, tag = "13")]
	pub payment_metadata: ::core::option::Option<::prost::alloc::string::String>,
	/// Whether the invoice has expired.
	#[prost(bool, tag = "15")]
	pub is_expired: bool,
}
/// Decode a BOLT12 offer and return its parsed fields.
/// This does not require a running node — it only parses the offer string.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DecodeOfferRequest {
	/// The BOLT12 offer string to decode.
	#[prost(string, tag = "1")]
	pub offer: ::prost::alloc::string::String,
}
/// The response for the `DecodeOffer` RPC. On failure, a gRPC error status is returned.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DecodeOfferResponse {
	/// The hex-encoded offer ID.
	#[prost(string, tag = "1")]
	pub offer_id: ::prost::alloc::string::String,
	/// The description of the offer, if any.
	#[prost(string, optional, tag = "2")]
	pub description: ::core::option::Option<::prost::alloc::string::String>,
	/// The issuer of the offer, if any.
	#[prost(string, optional, tag = "3")]
	pub issuer: ::core::option::Option<::prost::alloc::string::String>,
	/// The amount, if specified.
	#[prost(message, optional, tag = "4")]
	pub amount: ::core::option::Option<super::types::OfferAmount>,
	/// The hex-encoded public key used by the issuer to sign invoices, if any.
	#[prost(string, optional, tag = "5")]
	pub issuer_signing_pubkey: ::core::option::Option<::prost::alloc::string::String>,
	/// The absolute expiry time in seconds since the UNIX epoch, if any.
	#[prost(uint64, optional, tag = "6")]
	pub absolute_expiry: ::core::option::Option<u64>,
	/// The supported quantity of items.
	#[prost(message, optional, tag = "7")]
	pub quantity: ::core::option::Option<super::types::OfferQuantity>,
	/// Blinded paths to the offer recipient.
	#[prost(message, repeated, tag = "8")]
	pub paths: ::prost::alloc::vec::Vec<super::types::BlindedPath>,
	/// Feature bits advertised in the offer, keyed by bit number.
	#[prost(map = "uint32, message", tag = "9")]
	pub features: ::std::collections::HashMap<u32, super::types::Bolt11Feature>,
	/// Supported blockchain networks (e.g., "bitcoin", "testnet", "signet", "regtest").
	#[prost(string, repeated, tag = "10")]
	pub chains: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
	/// The metadata, hex-encoded, if any.
	#[prost(string, optional, tag = "11")]
	pub metadata: ::core::option::Option<::prost::alloc::string::String>,
	/// Whether the offer has expired.
	#[prost(bool, tag = "12")]
	pub is_expired: bool,
}
/// Subscribe to a stream of server events.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeEventsRequest {}
