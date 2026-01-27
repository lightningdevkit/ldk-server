// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Storage types for persisting payment data.
//!
//! These types are separate from the proto definitions to decouple the storage format
//! from the API format. This allows the storage schema to evolve independently and
//! provides better control over backwards compatibility.

use ldk_node::lightning::impl_writeable_tlv_based;
use ldk_node::lightning::routing::gossip::NodeId;

/// A forwarded payment stored in the database.
///
/// This type is needed because ldk-node doesn't persist forwarded payment events -
/// it only emits them. We need our own storage type to track forwarding history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredForwardedPayment {
	/// The channel id of the incoming channel.
	pub prev_channel_id: [u8; 32],
	/// The channel id of the outgoing channel.
	pub next_channel_id: [u8; 32],
	/// The user_channel_id of the incoming channel.
	pub prev_user_channel_id: u128,
	/// The user_channel_id of the outgoing channel.
	pub next_user_channel_id: Option<u128>,
	/// The node id of the previous node.
	pub prev_node_id: NodeId,
	/// The node id of the next node.
	pub next_node_id: NodeId,
	/// The total fee earned in millisatoshis.
	pub total_fee_earned_msat: Option<u64>,
	/// The skimmed fee in millisatoshis.
	pub skimmed_fee_msat: Option<u64>,
	/// Whether the payment was claimed from an on-chain transaction.
	pub claim_from_onchain_tx: bool,
	/// The outbound amount forwarded in millisatoshis.
	pub outbound_amount_forwarded_msat: Option<u64>,
}

impl_writeable_tlv_based!(StoredForwardedPayment, {
	(0, prev_channel_id, required),
	(2, next_channel_id, required),
	(4, prev_user_channel_id, required),
	(6, next_user_channel_id, option),
	(8, prev_node_id, required),
	(10, next_node_id, required),
	(12, total_fee_earned_msat, option),
	(14, skimmed_fee_msat, option),
	(16, claim_from_onchain_tx, required),
	(18, outbound_amount_forwarded_msat, option),
});
