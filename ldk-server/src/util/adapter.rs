// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use hex::prelude::*;
use hyper::StatusCode;
use ldk_node::bitcoin::hashes::sha256;
use ldk_node::bitcoin::hashes::Hash as _;
use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::config::{ChannelConfig, MaxDustHTLCExposure};
use ldk_node::lightning::chain::channelmonitor::BalanceSource;
use ldk_node::lightning::ln::types::ChannelId;
use ldk_node::lightning::routing::gossip::{
	ChannelInfo, ChannelUpdateInfo, NodeAnnouncementInfo, NodeInfo, RoutingFees,
};
use ldk_node::lightning_invoice::{Bolt11InvoiceDescription, Description, Sha256};
use ldk_node::payment::{
	ConfirmationStatus, PaymentDetails, PaymentDirection, PaymentKind, PaymentStatus,
};
use ldk_node::{ChannelDetails, LightningBalance, PeerDetails, PendingSweepBalance, UserChannelId};
use ldk_server_json_models::error::{ErrorCode, ErrorResponse};
use ldk_server_json_models::types::{
	Channel, ForwardedPayment, LspFeeLimits, OutPoint, Payment, Peer,
};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::{
	AuthError, InternalServerError, InvalidRequestError, LightningError,
};

pub(crate) fn peer_to_model(peer: PeerDetails) -> Peer {
	Peer {
		node_id: peer.node_id.serialize(),
		address: peer.address.to_string(),
		is_persisted: peer.is_persisted,
		is_connected: peer.is_connected,
	}
}

pub(crate) fn channel_to_model(channel: ChannelDetails) -> Channel {
	Channel {
		channel_id: channel.channel_id.0,
		counterparty_node_id: channel.counterparty_node_id.serialize(),
		funding_txo: channel
			.funding_txo
			.map(|o| OutPoint { txid: o.txid.to_byte_array(), vout: o.vout }),
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
		channel_config: Some(channel_config_to_model(channel.config)),
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

pub(crate) fn channel_config_to_model(
	channel_config: ChannelConfig,
) -> ldk_server_json_models::types::ChannelConfig {
	ldk_server_json_models::types::ChannelConfig {
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
				Some(ldk_server_json_models::types::MaxDustHtlcExposure::FixedLimitMsat(limit_msat))
			},
			MaxDustHTLCExposure::FeeRateMultiplier { multiplier } => Some(
				ldk_server_json_models::types::MaxDustHtlcExposure::FeeRateMultiplier(multiplier),
			),
		},
	}
}

pub(crate) fn payment_to_model(payment: PaymentDetails) -> Payment {
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
		id: id.0,
		kind: payment_kind_to_model(kind),
		amount_msat,
		fee_paid_msat,
		direction: match direction {
			PaymentDirection::Inbound => ldk_server_json_models::types::PaymentDirection::Inbound,
			PaymentDirection::Outbound => ldk_server_json_models::types::PaymentDirection::Outbound,
		},
		status: match status {
			PaymentStatus::Pending => ldk_server_json_models::types::PaymentStatus::Pending,
			PaymentStatus::Succeeded => ldk_server_json_models::types::PaymentStatus::Succeeded,
			PaymentStatus::Failed => ldk_server_json_models::types::PaymentStatus::Failed,
		},
		latest_update_timestamp,
	}
}

pub(crate) fn payment_kind_to_model(
	payment_kind: PaymentKind,
) -> ldk_server_json_models::types::PaymentKind {
	match payment_kind {
		PaymentKind::Onchain { txid, status } => {
			ldk_server_json_models::types::PaymentKind::Onchain(
				ldk_server_json_models::types::Onchain {
					txid: txid.to_byte_array(),
					status: confirmation_status_to_model(status),
				},
			)
		},
		PaymentKind::Bolt11 { hash, preimage, secret } => {
			ldk_server_json_models::types::PaymentKind::Bolt11(
				ldk_server_json_models::types::Bolt11 {
					hash: hash.0,
					preimage: preimage.map(|p| p.0),
					secret: secret.map(|s| s.0),
				},
			)
		},
		PaymentKind::Bolt11Jit {
			hash,
			preimage,
			secret,
			lsp_fee_limits,
			counterparty_skimmed_fee_msat,
		} => ldk_server_json_models::types::PaymentKind::Bolt11Jit(
			ldk_server_json_models::types::Bolt11Jit {
				hash: hash.0,
				preimage: preimage.map(|p| p.0),
				secret: secret.map(|s| s.0),
				lsp_fee_limits: Some(LspFeeLimits {
					max_total_opening_fee_msat: lsp_fee_limits.max_total_opening_fee_msat,
					max_proportional_opening_fee_ppm_msat: lsp_fee_limits
						.max_proportional_opening_fee_ppm_msat,
				}),
				counterparty_skimmed_fee_msat,
			},
		),
		PaymentKind::Bolt12Offer { hash, preimage, secret, offer_id, payer_note, quantity } => {
			ldk_server_json_models::types::PaymentKind::Bolt12Offer(
				ldk_server_json_models::types::Bolt12Offer {
					hash: hash.map(|h| h.0),
					preimage: preimage.map(|p| p.0),
					secret: secret.map(|s| s.0),
					offer_id: offer_id.0,
					payer_note: payer_note.map(|s| s.to_string()),
					quantity,
				},
			)
		},
		PaymentKind::Bolt12Refund { hash, preimage, secret, payer_note, quantity } => {
			ldk_server_json_models::types::PaymentKind::Bolt12Refund(
				ldk_server_json_models::types::Bolt12Refund {
					hash: hash.map(|h| h.0),
					preimage: preimage.map(|p| p.0),
					secret: secret.map(|s| s.0),
					payer_note: payer_note.map(|s| s.to_string()),
					quantity,
				},
			)
		},
		PaymentKind::Spontaneous { hash, preimage } => {
			ldk_server_json_models::types::PaymentKind::Spontaneous(
				ldk_server_json_models::types::Spontaneous {
					hash: hash.0,
					preimage: preimage.map(|p| p.0),
				},
			)
		},
	}
}

pub(crate) fn confirmation_status_to_model(
	confirmation_status: ConfirmationStatus,
) -> ldk_server_json_models::types::ConfirmationStatus {
	match confirmation_status {
		ConfirmationStatus::Confirmed { block_hash, height, timestamp } => {
			ldk_server_json_models::types::ConfirmationStatus::Confirmed(
				ldk_server_json_models::types::Confirmed {
					block_hash: block_hash.to_byte_array(),
					height,
					timestamp,
				},
			)
		},
		ConfirmationStatus::Unconfirmed => {
			ldk_server_json_models::types::ConfirmationStatus::Unconfirmed(
				ldk_server_json_models::types::Unconfirmed {},
			)
		},
	}
}

pub(crate) fn lightning_balance_to_model(
	lightning_balance: LightningBalance,
) -> ldk_server_json_models::types::LightningBalance {
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
		} => ldk_server_json_models::types::LightningBalance::ClaimableOnChannelClose(
			ldk_server_json_models::types::ClaimableOnChannelClose {
				channel_id: channel_id.0,
				counterparty_node_id: counterparty_node_id.serialize(),
				amount_satoshis,
				transaction_fee_satoshis,
				outbound_payment_htlc_rounded_msat,
				outbound_forwarded_htlc_rounded_msat,
				inbound_claiming_htlc_rounded_msat,
				inbound_htlc_rounded_msat,
			},
		),
		LightningBalance::ClaimableAwaitingConfirmations {
			channel_id,
			counterparty_node_id,
			amount_satoshis,
			confirmation_height,
			source,
		} => ldk_server_json_models::types::LightningBalance::ClaimableAwaitingConfirmations(
			ldk_server_json_models::types::ClaimableAwaitingConfirmations {
				channel_id: channel_id.0,
				counterparty_node_id: counterparty_node_id.serialize(),
				amount_satoshis,
				confirmation_height,
				source: match source {
					BalanceSource::HolderForceClosed => {
						ldk_server_json_models::types::BalanceSource::HolderForceClosed
					},
					BalanceSource::CounterpartyForceClosed => {
						ldk_server_json_models::types::BalanceSource::CounterpartyForceClosed
					},
					BalanceSource::CoopClose => {
						ldk_server_json_models::types::BalanceSource::CoopClose
					},
					BalanceSource::Htlc => ldk_server_json_models::types::BalanceSource::Htlc,
				},
			},
		),
		LightningBalance::ContentiousClaimable {
			channel_id,
			counterparty_node_id,
			amount_satoshis,
			timeout_height,
			payment_hash,
			payment_preimage,
		} => ldk_server_json_models::types::LightningBalance::ContentiousClaimable(
			ldk_server_json_models::types::ContentiousClaimable {
				channel_id: channel_id.0,
				counterparty_node_id: counterparty_node_id.serialize(),
				amount_satoshis,
				timeout_height,
				payment_hash: payment_hash.0,
				payment_preimage: payment_preimage.0,
			},
		),
		LightningBalance::MaybeTimeoutClaimableHTLC {
			channel_id,
			counterparty_node_id,
			amount_satoshis,
			claimable_height,
			payment_hash,
			outbound_payment,
		} => ldk_server_json_models::types::LightningBalance::MaybeTimeoutClaimableHtlc(
			ldk_server_json_models::types::MaybeTimeoutClaimableHtlc {
				channel_id: channel_id.0,
				counterparty_node_id: counterparty_node_id.serialize(),
				amount_satoshis,
				claimable_height,
				payment_hash: payment_hash.0,
				outbound_payment,
			},
		),
		LightningBalance::MaybePreimageClaimableHTLC {
			channel_id,
			counterparty_node_id,
			amount_satoshis,
			expiry_height,
			payment_hash,
		} => ldk_server_json_models::types::LightningBalance::MaybePreimageClaimableHtlc(
			ldk_server_json_models::types::MaybePreimageClaimableHtlc {
				channel_id: channel_id.0,
				counterparty_node_id: counterparty_node_id.serialize(),
				amount_satoshis,
				expiry_height,
				payment_hash: payment_hash.0,
			},
		),
		LightningBalance::CounterpartyRevokedOutputClaimable {
			channel_id,
			counterparty_node_id,
			amount_satoshis,
		} => ldk_server_json_models::types::LightningBalance::CounterpartyRevokedOutputClaimable(
			ldk_server_json_models::types::CounterpartyRevokedOutputClaimable {
				channel_id: channel_id.0,
				counterparty_node_id: counterparty_node_id.serialize(),
				amount_satoshis,
			},
		),
	}
}

pub(crate) fn pending_sweep_balance_to_model(
	pending_sweep_balance: PendingSweepBalance,
) -> ldk_server_json_models::types::PendingSweepBalance {
	match pending_sweep_balance {
		PendingSweepBalance::PendingBroadcast { channel_id, amount_satoshis } => {
			ldk_server_json_models::types::PendingSweepBalance::PendingBroadcast(
				ldk_server_json_models::types::PendingBroadcast {
					channel_id: channel_id.map(|c| c.0),
					amount_satoshis,
				},
			)
		},
		PendingSweepBalance::BroadcastAwaitingConfirmation {
			channel_id,
			latest_broadcast_height,
			latest_spending_txid,
			amount_satoshis,
		} => ldk_server_json_models::types::PendingSweepBalance::BroadcastAwaitingConfirmation(
			ldk_server_json_models::types::BroadcastAwaitingConfirmation {
				channel_id: channel_id.map(|c| c.0),
				latest_broadcast_height,
				latest_spending_txid: latest_spending_txid.to_byte_array(),
				amount_satoshis,
			},
		),
		PendingSweepBalance::AwaitingThresholdConfirmations {
			channel_id,
			latest_spending_txid,
			confirmation_hash,
			confirmation_height,
			amount_satoshis,
		} => ldk_server_json_models::types::PendingSweepBalance::AwaitingThresholdConfirmations(
			ldk_server_json_models::types::AwaitingThresholdConfirmations {
				channel_id: channel_id.map(|c| c.0),
				latest_spending_txid: latest_spending_txid.to_byte_array(),
				confirmation_hash: confirmation_hash.to_byte_array(),
				confirmation_height,
				amount_satoshis,
			},
		),
	}
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn forwarded_payment_to_model(
	prev_channel_id: ChannelId, next_channel_id: ChannelId,
	prev_user_channel_id: Option<UserChannelId>, next_user_channel_id: Option<UserChannelId>,
	prev_node_id: Option<PublicKey>, next_node_id: Option<PublicKey>,
	total_fee_earned_msat: Option<u64>, skimmed_fee_msat: Option<u64>, claim_from_onchain_tx: bool,
	outbound_amount_forwarded_msat: Option<u64>,
) -> ForwardedPayment {
	ForwardedPayment {
		prev_channel_id: prev_channel_id.0,
		next_channel_id: next_channel_id.0,
		prev_user_channel_id: prev_user_channel_id
			.expect("prev_user_channel_id expected for ldk-server >=0.1")
			.0
			.to_string(),
		next_user_channel_id: next_user_channel_id.map(|u| u.0.to_string()),
		prev_node_id: prev_node_id.expect("prev_node_id expected for ldk-server >=0.1").serialize(),
		next_node_id: next_node_id.expect("next_node_id expected for ldk-node >=0.1").serialize(),
		total_fee_earned_msat,
		skimmed_fee_msat,
		claim_from_onchain_tx,
		outbound_amount_forwarded_msat,
	}
}

pub(crate) fn bolt11_description_from_model(
	description: Option<ldk_server_json_models::types::Bolt11InvoiceDescription>,
) -> Result<Bolt11InvoiceDescription, LdkServerError> {
	Ok(match description {
		Some(ldk_server_json_models::types::Bolt11InvoiceDescription::Direct(s)) => {
			Bolt11InvoiceDescription::Direct(Description::new(s).map_err(|e| {
				LdkServerError::new(
					InvalidRequestError,
					format!("Invalid invoice description: {}", e),
				)
			})?)
		},
		Some(ldk_server_json_models::types::Bolt11InvoiceDescription::Hash(h)) => {
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

pub(crate) fn graph_routing_fees_to_model(
	fees: RoutingFees,
) -> ldk_server_json_models::types::GraphRoutingFees {
	ldk_server_json_models::types::GraphRoutingFees {
		base_msat: fees.base_msat,
		proportional_millionths: fees.proportional_millionths,
	}
}

pub(crate) fn graph_channel_update_to_model(
	update: ChannelUpdateInfo,
) -> ldk_server_json_models::types::GraphChannelUpdate {
	ldk_server_json_models::types::GraphChannelUpdate {
		last_update: update.last_update,
		enabled: update.enabled,
		cltv_expiry_delta: update.cltv_expiry_delta as u32,
		htlc_minimum_msat: update.htlc_minimum_msat,
		htlc_maximum_msat: update.htlc_maximum_msat,
		fees: graph_routing_fees_to_model(update.fees),
	}
}

pub(crate) fn graph_channel_to_model(
	channel: ChannelInfo,
) -> ldk_server_json_models::types::GraphChannel {
	ldk_server_json_models::types::GraphChannel {
		node_one: channel.node_one.as_slice().try_into().expect("NodeId should be 33 bytes"),
		node_two: channel.node_two.as_slice().try_into().expect("NodeId should be 33 bytes"),
		capacity_sats: channel.capacity_sats,
		one_to_two: channel.one_to_two.map(graph_channel_update_to_model),
		two_to_one: channel.two_to_one.map(graph_channel_update_to_model),
	}
}

pub(crate) fn graph_node_announcement_to_model(
	announcement: NodeAnnouncementInfo,
) -> ldk_server_json_models::types::GraphNodeAnnouncement {
	let rgb = announcement.rgb();
	ldk_server_json_models::types::GraphNodeAnnouncement {
		last_update: announcement.last_update(),
		alias: announcement.alias().to_string(),
		rgb: format!("{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]),
		addresses: announcement.addresses().iter().map(|a| a.to_string()).collect(),
	}
}

pub(crate) fn graph_node_to_model(node: NodeInfo) -> ldk_server_json_models::types::GraphNode {
	ldk_server_json_models::types::GraphNode {
		channels: node.channels,
		announcement_info: node.announcement_info.map(graph_node_announcement_to_model),
	}
}

pub(crate) fn to_error_response(ldk_error: LdkServerError) -> (ErrorResponse, StatusCode) {
	let error_code = match ldk_error.error_code {
		InvalidRequestError => ErrorCode::InvalidRequestError,
		AuthError => ErrorCode::AuthError,
		LightningError => ErrorCode::LightningError,
		InternalServerError => ErrorCode::InternalServerError,
	};

	let status = match ldk_error.error_code {
		InvalidRequestError => StatusCode::BAD_REQUEST,
		AuthError => StatusCode::UNAUTHORIZED,
		LightningError => StatusCode::INTERNAL_SERVER_ERROR,
		InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
	};

	let error_response = ErrorResponse { message: ldk_error.message, error_code };

	(error_response, status)
}
