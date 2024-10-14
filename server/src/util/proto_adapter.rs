use bytes::Bytes;
use hex::prelude::*;
use ldk_node::config::{ChannelConfig, MaxDustHTLCExposure};
use ldk_node::payment::{PaymentDetails, PaymentDirection, PaymentKind, PaymentStatus};
use ldk_node::ChannelDetails;
use protos::payment_kind::Kind::{
	Bolt11, Bolt11Jit, Bolt12Offer, Bolt12Refund, Onchain, Spontaneous,
};
use protos::{Channel, LspFeeLimits, OutPoint, Payment};

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

pub(crate) fn channel_config_to_proto(channel_config: ChannelConfig) -> protos::ChannelConfig {
	protos::ChannelConfig {
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
				Some(protos::channel_config::MaxDustHtlcExposure::FixedLimitMsat(limit_msat))
			},
			MaxDustHTLCExposure::FeeRateMultiplier { multiplier } => {
				Some(protos::channel_config::MaxDustHtlcExposure::FeeRateMultiplier(multiplier))
			},
		},
	}
}

pub(crate) fn payment_to_proto(payment: PaymentDetails) -> Payment {
	Payment {
		id: payment.id.0.to_lower_hex_string(),
		kind: Some(payment_kind_to_proto(payment.kind)),
		amount_msat: payment.amount_msat,
		direction: match payment.direction {
			PaymentDirection::Inbound => protos::PaymentDirection::Inbound.into(),
			PaymentDirection::Outbound => protos::PaymentDirection::Outbound.into(),
		},
		status: match payment.status {
			PaymentStatus::Pending => protos::PaymentStatus::Pending.into(),
			PaymentStatus::Succeeded => protos::PaymentStatus::Succeeded.into(),
			PaymentStatus::Failed => protos::PaymentStatus::Failed.into(),
		},
		latest_update_timestamp: payment.latest_update_timestamp,
	}
}

pub(crate) fn payment_kind_to_proto(payment_kind: PaymentKind) -> protos::PaymentKind {
	match payment_kind {
		PaymentKind::Onchain => protos::PaymentKind { kind: Some(Onchain(protos::Onchain {})) },
		PaymentKind::Bolt11 { hash, preimage, secret } => protos::PaymentKind {
			kind: Some(Bolt11(protos::Bolt11 {
				hash: hash.to_string(),
				preimage: preimage.map(|p| p.to_string()),
				secret: secret.map(|s| Bytes::copy_from_slice(&s.0)),
			})),
		},
		PaymentKind::Bolt11Jit { hash, preimage, secret, lsp_fee_limits } => protos::PaymentKind {
			kind: Some(Bolt11Jit(protos::Bolt11Jit {
				hash: hash.to_string(),
				preimage: preimage.map(|p| p.to_string()),
				secret: secret.map(|s| Bytes::copy_from_slice(&s.0)),
				lsp_fee_limits: Some(LspFeeLimits {
					max_total_opening_fee_msat: lsp_fee_limits.max_total_opening_fee_msat,
					max_proportional_opening_fee_ppm_msat: lsp_fee_limits
						.max_proportional_opening_fee_ppm_msat,
				}),
			})),
		},
		PaymentKind::Bolt12Offer { hash, preimage, secret, offer_id, payer_note, quantity } => {
			protos::PaymentKind {
				kind: Some(Bolt12Offer(protos::Bolt12Offer {
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
			protos::PaymentKind {
				kind: Some(Bolt12Refund(protos::Bolt12Refund {
					hash: hash.map(|h| h.to_string()),
					preimage: preimage.map(|p| p.to_string()),
					secret: secret.map(|s| Bytes::copy_from_slice(&s.0)),
					payer_note: payer_note.map(|s| s.to_string()),
					quantity,
				})),
			}
		},
		PaymentKind::Spontaneous { hash, preimage } => protos::PaymentKind {
			kind: Some(Spontaneous(protos::Spontaneous {
				hash: hash.to_string(),
				preimage: preimage.map(|p| p.to_string()),
			})),
		},
	}
}
