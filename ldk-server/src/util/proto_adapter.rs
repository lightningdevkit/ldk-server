// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::collections::BTreeMap;

use bytes::Bytes;
use hex::prelude::*;
use ldk_node::bitcoin::hashes::sha256;
use ldk_node::bitcoin::Network;
use ldk_node::config::{ChannelConfig, MaxDustHTLCExposure};
use ldk_node::lightning::chain::channelmonitor::BalanceSource;
use ldk_node::lightning::routing::gossip::{
	ChannelInfo, ChannelUpdateInfo, NodeAnnouncementInfo, NodeInfo, RoutingFees,
};
use ldk_node::lightning_invoice::{Bolt11InvoiceDescription, Description, Sha256};
use ldk_node::lightning_types::features::NodeFeatures;
use ldk_node::payment::{
	ConfirmationStatus, PaymentDetails, PaymentDirection, PaymentKind, PaymentStatus,
};
use ldk_node::{ChannelDetails, LightningBalance, PeerDetails, PendingSweepBalance};
use ldk_server_grpc::types::confirmation_status::Status::{Confirmed, Unconfirmed};
use ldk_server_grpc::types::lightning_balance::BalanceType::{
	ClaimableAwaitingConfirmations, ClaimableOnChannelClose, ContentiousClaimable,
	CounterpartyRevokedOutputClaimable, MaybePreimageClaimableHtlc, MaybeTimeoutClaimableHtlc,
};
use ldk_server_grpc::types::payment_kind::Kind::{
	Bolt11, Bolt12Offer, Bolt12Refund, Onchain, Spontaneous,
};
use ldk_server_grpc::types::pending_sweep_balance::BalanceType::{
	AwaitingThresholdConfirmations, BroadcastAwaitingConfirmation, PendingBroadcast,
};
use ldk_server_grpc::types::{
	bolt11_invoice_description, Channel, Feature, ForwardedPayment, HtlcLocator, OutPoint, Payment,
	Peer,
};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;

pub(crate) fn peer_to_proto(peer: PeerDetails) -> Peer {
	Peer {
		node_id: peer.node_id.to_string(),
		address: peer.address.to_string(),
		is_persisted: peer.is_persisted,
		is_connected: peer.is_connected,
	}
}

pub(crate) fn channel_to_proto(channel: ChannelDetails) -> Channel {
	Channel {
		channel_id: channel.channel_id.0.to_lower_hex_string(),
		counterparty_node_id: channel.counterparty_node_id.to_string(),
		funding_txo: channel
			.funding_txo
			.map(|o| OutPoint { txid: o.txid.to_string(), vout: o.vout }),
		user_channel_id: channel.user_channel_id.0.to_string(),
		unspendable_punishment_reserve: channel.unspendable_punishment_reserve,
		channel_value_sats: channel.channel_value_sats,
		feerate_sat_per_1000_weight: channel.feerate_sat_per_1000_weight,
		outbound_capacity_msat: channel.outbound_capacity_msat,
		inbound_capacity_msat: channel.inbound_capacity_msat,
		confirmations_required: channel.confirmations_required,
		confirmations: channel.confirmations,
		is_outbound: channel.is_outbound,
		is_channel_ready: channel.is_channel_ready,
		is_usable: channel.is_usable,
		is_announced: channel.is_announced,
		channel_config: Some(channel_config_to_proto(channel.config)),
		next_outbound_htlc_limit_msat: channel.next_outbound_htlc_limit_msat,
		next_outbound_htlc_minimum_msat: channel.next_outbound_htlc_minimum_msat,
		force_close_spend_delay: channel.force_close_spend_delay.map(|x| x as u32),
		counterparty_outbound_htlc_minimum_msat: channel.counterparty_outbound_htlc_minimum_msat,
		counterparty_outbound_htlc_maximum_msat: channel.counterparty_outbound_htlc_maximum_msat,
		counterparty_unspendable_punishment_reserve: channel
			.counterparty_unspendable_punishment_reserve,
		counterparty_forwarding_info_fee_base_msat: channel
			.counterparty_forwarding_info_fee_base_msat,
		counterparty_forwarding_info_fee_proportional_millionths: channel
			.counterparty_forwarding_info_fee_proportional_millionths,
		counterparty_forwarding_info_cltv_expiry_delta: channel
			.counterparty_forwarding_info_cltv_expiry_delta
			.map(|x| x as u32),
	}
}

pub(crate) fn channel_config_to_proto(
	channel_config: ChannelConfig,
) -> ldk_server_grpc::types::ChannelConfig {
	ldk_server_grpc::types::ChannelConfig {
		forwarding_fee_proportional_millionths: Some(
			channel_config.forwarding_fee_proportional_millionths,
		),
		forwarding_fee_base_msat: Some(channel_config.forwarding_fee_base_msat),
		cltv_expiry_delta: Some(channel_config.cltv_expiry_delta as u32),
		force_close_avoidance_max_fee_satoshis: Some(
			channel_config.force_close_avoidance_max_fee_satoshis,
		),
		accept_underpaying_htlcs: Some(channel_config.accept_underpaying_htlcs),
		max_dust_htlc_exposure: match channel_config.max_dust_htlc_exposure {
			MaxDustHTLCExposure::FixedLimit { limit_msat } => {
				Some(ldk_server_grpc::types::channel_config::MaxDustHtlcExposure::FixedLimitMsat(
					limit_msat,
				))
			},
			MaxDustHTLCExposure::FeeRateMultiplier { multiplier } => Some(
				ldk_server_grpc::types::channel_config::MaxDustHtlcExposure::FeeRateMultiplier(
					multiplier,
				),
			),
		},
	}
}

pub(crate) fn payment_to_proto(payment: PaymentDetails) -> Payment {
	let PaymentDetails {
		id,
		kind,
		amount_msat,
		fee_paid_msat,
		direction,
		status,
		latest_update_timestamp,
	} = payment;

	Payment {
		id: id.to_string(),
		kind: Some(payment_kind_to_proto(kind)),
		amount_msat,
		fee_paid_msat,
		direction: match direction {
			PaymentDirection::Inbound => ldk_server_grpc::types::PaymentDirection::Inbound.into(),
			PaymentDirection::Outbound => ldk_server_grpc::types::PaymentDirection::Outbound.into(),
		},
		status: match status {
			PaymentStatus::Pending => ldk_server_grpc::types::PaymentStatus::Pending.into(),
			PaymentStatus::Succeeded => ldk_server_grpc::types::PaymentStatus::Succeeded.into(),
			PaymentStatus::Failed => ldk_server_grpc::types::PaymentStatus::Failed.into(),
		},
		latest_update_timestamp,
	}
}

pub(crate) fn payment_kind_to_proto(
	payment_kind: PaymentKind,
) -> ldk_server_grpc::types::PaymentKind {
	match payment_kind {
		PaymentKind::Onchain { txid, status } => ldk_server_grpc::types::PaymentKind {
			kind: Some(Onchain(ldk_server_grpc::types::Onchain {
				txid: txid.to_string(),
				status: Some(confirmation_status_to_proto(status)),
			})),
		},
		PaymentKind::Bolt11 { hash, preimage, secret, counterparty_skimmed_fee_msat } => {
			ldk_server_grpc::types::PaymentKind {
				kind: Some(Bolt11(ldk_server_grpc::types::Bolt11 {
					hash: hash.to_string(),
					preimage: preimage.map(|p| p.to_string()),
					secret: secret.map(|s| Bytes::copy_from_slice(&s.0)),
					counterparty_skimmed_fee_msat,
				})),
			}
		},
		PaymentKind::Bolt12Offer { hash, preimage, secret, offer_id, payer_note, quantity } => {
			ldk_server_grpc::types::PaymentKind {
				kind: Some(Bolt12Offer(ldk_server_grpc::types::Bolt12Offer {
					hash: hash.map(|h| h.to_string()),
					preimage: preimage.map(|p| p.to_string()),
					secret: secret.map(|s| Bytes::copy_from_slice(&s.0)),
					offer_id: offer_id.0.to_lower_hex_string(),
					payer_note: payer_note.map(|s| s.to_string()),
					quantity,
				})),
			}
		},
		PaymentKind::Bolt12Refund { hash, preimage, secret, payer_note, quantity } => {
			ldk_server_grpc::types::PaymentKind {
				kind: Some(Bolt12Refund(ldk_server_grpc::types::Bolt12Refund {
					hash: hash.map(|h| h.to_string()),
					preimage: preimage.map(|p| p.to_string()),
					secret: secret.map(|s| Bytes::copy_from_slice(&s.0)),
					payer_note: payer_note.map(|s| s.to_string()),
					quantity,
				})),
			}
		},
		PaymentKind::Spontaneous { hash, preimage } => ldk_server_grpc::types::PaymentKind {
			kind: Some(Spontaneous(ldk_server_grpc::types::Spontaneous {
				hash: hash.to_string(),
				preimage: preimage.map(|p| p.to_string()),
			})),
		},
	}
}

pub(crate) fn confirmation_status_to_proto(
	confirmation_status: ConfirmationStatus,
) -> ldk_server_grpc::types::ConfirmationStatus {
	match confirmation_status {
		ConfirmationStatus::Confirmed { block_hash, height, timestamp } => {
			ldk_server_grpc::types::ConfirmationStatus {
				status: Some(Confirmed(ldk_server_grpc::types::Confirmed {
					block_hash: block_hash.to_string(),
					height,
					timestamp,
				})),
			}
		},
		ConfirmationStatus::Unconfirmed => ldk_server_grpc::types::ConfirmationStatus {
			status: Some(Unconfirmed(ldk_server_grpc::types::Unconfirmed {})),
		},
	}
}

pub(crate) fn lightning_balance_to_proto(
	lightning_balance: LightningBalance,
) -> ldk_server_grpc::types::LightningBalance {
	match lightning_balance {
		LightningBalance::ClaimableOnChannelClose {
			channel_id,
			counterparty_node_id,
			amount_satoshis,
			transaction_fee_satoshis,
			outbound_payment_htlc_rounded_msat,
			outbound_forwarded_htlc_rounded_msat,
			inbound_claiming_htlc_rounded_msat,
			inbound_htlc_rounded_msat,
		} => ldk_server_grpc::types::LightningBalance {
			balance_type: Some(ClaimableOnChannelClose(
				ldk_server_grpc::types::ClaimableOnChannelClose {
					channel_id: channel_id.0.to_lower_hex_string(),
					counterparty_node_id: counterparty_node_id.to_string(),
					amount_satoshis,
					transaction_fee_satoshis,
					outbound_payment_htlc_rounded_msat,
					outbound_forwarded_htlc_rounded_msat,
					inbound_claiming_htlc_rounded_msat,
					inbound_htlc_rounded_msat,
				},
			)),
		},
		LightningBalance::ClaimableAwaitingConfirmations {
			channel_id,
			counterparty_node_id,
			amount_satoshis,
			confirmation_height,
			source,
		} => ldk_server_grpc::types::LightningBalance {
			balance_type: Some(ClaimableAwaitingConfirmations(
				ldk_server_grpc::types::ClaimableAwaitingConfirmations {
					channel_id: channel_id.0.to_lower_hex_string(),
					counterparty_node_id: counterparty_node_id.to_string(),
					amount_satoshis,
					confirmation_height,
					source: match source {
						BalanceSource::HolderForceClosed => {
							ldk_server_grpc::types::BalanceSource::HolderForceClosed.into()
						},
						BalanceSource::CounterpartyForceClosed => {
							ldk_server_grpc::types::BalanceSource::CounterpartyForceClosed.into()
						},
						BalanceSource::CoopClose => {
							ldk_server_grpc::types::BalanceSource::CoopClose.into()
						},
						BalanceSource::Htlc => ldk_server_grpc::types::BalanceSource::Htlc.into(),
					},
				},
			)),
		},
		LightningBalance::ContentiousClaimable {
			channel_id,
			counterparty_node_id,
			amount_satoshis,
			timeout_height,
			payment_hash,
			payment_preimage,
		} => ldk_server_grpc::types::LightningBalance {
			balance_type: Some(ContentiousClaimable(
				ldk_server_grpc::types::ContentiousClaimable {
					channel_id: channel_id.0.to_lower_hex_string(),
					counterparty_node_id: counterparty_node_id.to_string(),
					amount_satoshis,
					timeout_height,
					payment_hash: payment_hash.to_string(),
					payment_preimage: payment_preimage.to_string(),
				},
			)),
		},
		LightningBalance::MaybeTimeoutClaimableHTLC {
			channel_id,
			counterparty_node_id,
			amount_satoshis,
			claimable_height,
			payment_hash,
			outbound_payment,
		} => ldk_server_grpc::types::LightningBalance {
			balance_type: Some(MaybeTimeoutClaimableHtlc(
				ldk_server_grpc::types::MaybeTimeoutClaimableHtlc {
					channel_id: channel_id.0.to_lower_hex_string(),
					counterparty_node_id: counterparty_node_id.to_string(),
					amount_satoshis,
					claimable_height,
					payment_hash: payment_hash.to_string(),
					outbound_payment,
				},
			)),
		},
		LightningBalance::MaybePreimageClaimableHTLC {
			channel_id,
			counterparty_node_id,
			amount_satoshis,
			expiry_height,
			payment_hash,
		} => ldk_server_grpc::types::LightningBalance {
			balance_type: Some(MaybePreimageClaimableHtlc(
				ldk_server_grpc::types::MaybePreimageClaimableHtlc {
					channel_id: channel_id.0.to_lower_hex_string(),
					counterparty_node_id: counterparty_node_id.to_string(),
					amount_satoshis,
					expiry_height,
					payment_hash: payment_hash.to_string(),
				},
			)),
		},
		LightningBalance::CounterpartyRevokedOutputClaimable {
			channel_id,
			counterparty_node_id,
			amount_satoshis,
		} => ldk_server_grpc::types::LightningBalance {
			balance_type: Some(CounterpartyRevokedOutputClaimable(
				ldk_server_grpc::types::CounterpartyRevokedOutputClaimable {
					channel_id: channel_id.0.to_lower_hex_string(),
					counterparty_node_id: counterparty_node_id.to_string(),
					amount_satoshis,
				},
			)),
		},
	}
}

pub(crate) fn pending_sweep_balance_to_proto(
	pending_sweep_balance: PendingSweepBalance,
) -> ldk_server_grpc::types::PendingSweepBalance {
	match pending_sweep_balance {
		PendingSweepBalance::PendingBroadcast { channel_id, amount_satoshis } => {
			ldk_server_grpc::types::PendingSweepBalance {
				balance_type: Some(PendingBroadcast(ldk_server_grpc::types::PendingBroadcast {
					channel_id: channel_id.map(|c| c.0.to_lower_hex_string()),
					amount_satoshis,
				})),
			}
		},
		PendingSweepBalance::BroadcastAwaitingConfirmation {
			channel_id,
			latest_broadcast_height,
			latest_spending_txid,
			amount_satoshis,
		} => ldk_server_grpc::types::PendingSweepBalance {
			balance_type: Some(BroadcastAwaitingConfirmation(
				ldk_server_grpc::types::BroadcastAwaitingConfirmation {
					channel_id: channel_id.map(|c| c.0.to_lower_hex_string()),
					latest_broadcast_height,
					latest_spending_txid: latest_spending_txid.to_string(),
					amount_satoshis,
				},
			)),
		},
		PendingSweepBalance::AwaitingThresholdConfirmations {
			channel_id,
			latest_spending_txid,
			confirmation_hash,
			confirmation_height,
			amount_satoshis,
		} => ldk_server_grpc::types::PendingSweepBalance {
			balance_type: Some(AwaitingThresholdConfirmations(
				ldk_server_grpc::types::AwaitingThresholdConfirmations {
					channel_id: channel_id.map(|c| c.0.to_lower_hex_string()),
					latest_spending_txid: latest_spending_txid.to_string(),
					confirmation_hash: confirmation_hash.to_string(),
					confirmation_height,
					amount_satoshis,
				},
			)),
		},
	}
}

pub(crate) fn forwarded_payment_to_proto(
	prev_htlcs: Vec<HtlcLocator>, next_htlcs: Vec<HtlcLocator>, total_fee_earned_msat: Option<u64>,
	skimmed_fee_msat: Option<u64>, claim_from_onchain_tx: bool,
	outbound_amount_forwarded_msat: Option<u64>,
) -> ForwardedPayment {
	ForwardedPayment {
		total_fee_earned_msat,
		skimmed_fee_msat,
		claim_from_onchain_tx,
		outbound_amount_forwarded_msat,
		prev_htlcs,
		next_htlcs,
	}
}

pub(crate) fn proto_to_bolt11_description(
	description: Option<ldk_server_grpc::types::Bolt11InvoiceDescription>,
) -> Result<Bolt11InvoiceDescription, LdkServerError> {
	Ok(match description.and_then(|d| d.kind) {
		Some(bolt11_invoice_description::Kind::Direct(s)) => {
			Bolt11InvoiceDescription::Direct(Description::new(s).map_err(|e| {
				LdkServerError::new(
					InvalidRequestError,
					format!("Invalid invoice description: {}", e),
				)
			})?)
		},
		Some(bolt11_invoice_description::Kind::Hash(h)) => {
			let hash_bytes = <[u8; 32]>::from_hex(&h).map_err(|_| {
				LdkServerError::new(
					InvalidRequestError,
					"Invalid invoice description_hash, must be 32-byte hex string".to_string(),
				)
			})?;
			Bolt11InvoiceDescription::Hash(Sha256(*sha256::Hash::from_bytes_ref(&hash_bytes)))
		},
		None => {
			Bolt11InvoiceDescription::Direct(Description::new("".to_string()).map_err(|e| {
				LdkServerError::new(
					InvalidRequestError,
					format!("Invalid invoice description: {}", e),
				)
			})?)
		},
	})
}

pub(crate) fn graph_routing_fees_to_proto(
	fees: RoutingFees,
) -> ldk_server_grpc::types::GraphRoutingFees {
	ldk_server_grpc::types::GraphRoutingFees {
		base_msat: fees.base_msat,
		proportional_millionths: fees.proportional_millionths,
	}
}

pub(crate) fn graph_channel_update_to_proto(
	update: ChannelUpdateInfo,
) -> ldk_server_grpc::types::GraphChannelUpdate {
	ldk_server_grpc::types::GraphChannelUpdate {
		last_update: update.last_update,
		enabled: update.enabled,
		cltv_expiry_delta: update.cltv_expiry_delta as u32,
		htlc_minimum_msat: update.htlc_minimum_msat,
		htlc_maximum_msat: update.htlc_maximum_msat,
		fees: Some(graph_routing_fees_to_proto(update.fees)),
	}
}

pub(crate) fn graph_channel_to_proto(channel: ChannelInfo) -> ldk_server_grpc::types::GraphChannel {
	ldk_server_grpc::types::GraphChannel {
		node_one: channel.node_one.to_string(),
		node_two: channel.node_two.to_string(),
		capacity_sats: channel.capacity_sats,
		one_to_two: channel.one_to_two.map(graph_channel_update_to_proto),
		two_to_one: channel.two_to_one.map(graph_channel_update_to_proto),
	}
}

pub(crate) fn graph_node_announcement_to_proto(
	announcement: NodeAnnouncementInfo,
) -> ldk_server_grpc::types::GraphNodeAnnouncement {
	let rgb = announcement.rgb();
	let features = features_to_proto(announcement.features().le_flags(), |bytes| {
		NodeFeatures::from_le_bytes(bytes).to_string()
	});

	ldk_server_grpc::types::GraphNodeAnnouncement {
		last_update: announcement.last_update(),
		alias: announcement.alias().to_string(),
		rgb: format!("{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]),
		addresses: announcement.addresses().iter().map(|a| a.to_string()).collect(),
		features,
	}
}

pub(crate) fn graph_node_to_proto(node: NodeInfo) -> ldk_server_grpc::types::GraphNode {
	ldk_server_grpc::types::GraphNode {
		channels: node.channels,
		announcement_info: node.announcement_info.map(graph_node_announcement_to_proto),
	}
}

/// Converts LDK feature flags into proto features keyed by the signaled bit.
///
/// Feature names are derived from LDK's `Features::Display` impl, so they stay
/// in sync automatically. Unknown feature bits are skipped because returned
/// feature entries represent decoded features known by LDK.
pub(crate) fn features_to_proto(
	le_flags: &[u8], make_display: impl Fn(Vec<u8>) -> String,
) -> BTreeMap<u32, Feature> {
	let mut features = BTreeMap::new();

	for (byte_idx, &byte) in le_flags.iter().enumerate() {
		if byte == 0 {
			continue;
		}

		for bit_pos in 0..8u32 {
			if byte & (1 << bit_pos) == 0 {
				continue;
			}

			let bit_number = (byte_idx as u32) * 8 + bit_pos;

			// Create Features with just this bit set and use Display to get the name.
			let mut single_bit = vec![0u8; byte_idx + 1];
			single_bit[byte_idx] = 1 << bit_pos;

			let display = make_display(single_bit);
			let (name, is_known) = parse_feature_name(&display);
			if !is_known {
				continue;
			}

			features.insert(
				bit_number,
				Feature { name: name.to_string(), is_required: bit_number % 2 == 0 },
			);
		}
	}

	features
}

/// Parse the Display output of a single-bit Features to find which feature is set.
///
/// LDK's Display format is: "Name: status, Name: status, ..., unknown flags: status"
/// where status is "required", "supported", or "not supported".
/// For a single-bit Features, exactly one entry will be "required" or "supported".
fn parse_feature_name(display: &str) -> (&str, bool) {
	for entry in display.split(", ") {
		if let Some((name, status)) = entry.split_once(": ") {
			if name == "unknown flags" {
				if status == "required" || status == "supported" {
					return ("unknown", false);
				}
			} else if status == "required" || status == "supported" {
				return (name, true);
			}
		}
	}
	("unknown", false)
}

pub(crate) fn network_to_proto(network: Network) -> ldk_server_grpc::types::Network {
	use ldk_server_grpc::types::Network as ProtoNetwork;
	match network {
		Network::Bitcoin => ProtoNetwork::Bitcoin,
		Network::Testnet => ProtoNetwork::Testnet,
		Network::Testnet4 => ProtoNetwork::Testnet4,
		Network::Signet => ProtoNetwork::Signet,
		Network::Regtest => ProtoNetwork::Regtest,
	}
}
