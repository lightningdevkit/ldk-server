// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Serialization round-trip tests for all JSON models.
//!
//! Each test constructs a value, serializes it to JSON, deserializes back,
//! and asserts equality.  Key serde attributes (hex encoding, rename_all,
//! tagged enums) are spot-checked against the raw JSON.

use std::fmt::Debug;

use ldk_server_json_models::api::*;
use ldk_server_json_models::events::*;
use ldk_server_json_models::types::*;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Serialize to JSON and back, asserting round-trip equality.
fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T) -> serde_json::Value {
	let json = serde_json::to_value(value).expect("serialize");
	let back: T = serde_json::from_value(json.clone()).expect("deserialize");
	assert_eq!(&back, value, "round-trip mismatch");
	json
}

// ---------------------------------------------------------------------------
// Test data helpers
// ---------------------------------------------------------------------------

const HASH_32: [u8; 32] = [
	0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89,
	0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89,
];
const HASH_32_HEX: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

const HASH_32_B: [u8; 32] = [0x11; 32];
const HASH_32_B_HEX: &str = "1111111111111111111111111111111111111111111111111111111111111111";

const HASH_32_C: [u8; 32] = [0x22; 32];

const PUBKEY_33: [u8; 33] = [
	0x02, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
	0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
	0x89,
];
const PUBKEY_33_HEX: &str = "02abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

const PUBKEY_33_B: [u8; 33] = [0x03; 33];

fn sample_payment() -> Payment {
	Payment {
		id: HASH_32,
		kind: PaymentKind::Bolt11(Bolt11 {
			hash: HASH_32_B,
			preimage: Some(HASH_32_C),
			secret: None,
		}),
		amount_msat: Some(100_000),
		fee_paid_msat: Some(10),
		direction: PaymentDirection::Outbound,
		status: PaymentStatus::Succeeded,
		latest_update_timestamp: 1_700_000_000,
	}
}

fn sample_forwarded_payment() -> ForwardedPayment {
	ForwardedPayment {
		prev_channel_id: HASH_32,
		next_channel_id: HASH_32_B,
		prev_user_channel_id: "abc123".into(),
		prev_node_id: PUBKEY_33,
		next_node_id: PUBKEY_33_B,
		next_user_channel_id: Some("def456".into()),
		total_fee_earned_msat: Some(500),
		skimmed_fee_msat: None,
		claim_from_onchain_tx: false,
		outbound_amount_forwarded_msat: Some(99_500),
	}
}

fn sample_channel_config() -> ChannelConfig {
	ChannelConfig {
		forwarding_fee_proportional_millionths: Some(100),
		forwarding_fee_base_msat: Some(1000),
		cltv_expiry_delta: Some(144),
		force_close_avoidance_max_fee_satoshis: Some(1000),
		accept_underpaying_htlcs: Some(false),
		max_dust_htlc_exposure: Some(MaxDustHtlcExposure::FixedLimitMsat(5_000_000)),
	}
}

fn sample_channel() -> Channel {
	Channel {
		channel_id: HASH_32,
		counterparty_node_id: PUBKEY_33,
		funding_txo: Some(OutPoint { txid: HASH_32_B, vout: 0 }),
		user_channel_id: "user123".into(),
		unspendable_punishment_reserve: Some(1000),
		channel_value_sats: 1_000_000,
		feerate_sat_per_1000_weight: 253,
		outbound_capacity_msat: 500_000_000,
		inbound_capacity_msat: 400_000_000,
		confirmations_required: Some(3),
		confirmations: Some(6),
		is_outbound: true,
		is_channel_ready: true,
		is_usable: true,
		is_announced: false,
		channel_config: Some(sample_channel_config()),
		next_outbound_htlc_limit_msat: 450_000_000,
		next_outbound_htlc_minimum_msat: 1000,
		force_close_spend_delay: Some(144),
		counterparty_outbound_htlc_minimum_msat: Some(1000),
		counterparty_outbound_htlc_maximum_msat: Some(450_000_000),
		counterparty_unspendable_punishment_reserve: 1000,
		counterparty_forwarding_info_fee_base_msat: Some(1000),
		counterparty_forwarding_info_fee_proportional_millionths: Some(100),
		counterparty_forwarding_info_cltv_expiry_delta: Some(40),
	}
}

fn sample_route_params() -> RouteParametersConfig {
	RouteParametersConfig {
		max_total_routing_fee_msat: Some(5000),
		max_total_cltv_expiry_delta: 1008,
		max_path_count: 10,
		max_channel_saturation_power_of_half: 2,
	}
}

// ===========================================================================
// types.rs
// ===========================================================================

#[test]
fn payment_bolt11_roundtrip() {
	let p = sample_payment();
	let json = roundtrip(&p);
	assert_eq!(json["id"], HASH_32_HEX);
	assert_eq!(json["direction"], "outbound");
	assert_eq!(json["status"], "succeeded");
	assert!(json["kind"]["bolt11"].is_object());
	assert_eq!(json["kind"]["bolt11"]["hash"], HASH_32_B_HEX);
}

#[test]
fn payment_onchain_confirmed_roundtrip() {
	let p = Payment {
		id: HASH_32,
		kind: PaymentKind::Onchain(Onchain {
			txid: HASH_32_B,
			status: ConfirmationStatus::Confirmed(Confirmed {
				block_hash: HASH_32_C,
				height: 800_000,
				timestamp: 1_700_000_000,
			}),
		}),
		amount_msat: Some(50_000_000),
		fee_paid_msat: None,
		direction: PaymentDirection::Inbound,
		status: PaymentStatus::Succeeded,
		latest_update_timestamp: 1_700_000_000,
	};
	let json = roundtrip(&p);
	assert!(json["kind"]["onchain"]["status"]["confirmed"].is_object());
	assert_eq!(json["direction"], "inbound");
}

#[test]
fn payment_onchain_unconfirmed_roundtrip() {
	let p = Payment {
		id: HASH_32,
		kind: PaymentKind::Onchain(Onchain {
			txid: HASH_32_B,
			status: ConfirmationStatus::Unconfirmed(Unconfirmed {}),
		}),
		amount_msat: None,
		fee_paid_msat: None,
		direction: PaymentDirection::Outbound,
		status: PaymentStatus::Pending,
		latest_update_timestamp: 1_700_000_000,
	};
	let json = roundtrip(&p);
	assert!(json["kind"]["onchain"]["status"]["unconfirmed"].is_object());
	assert_eq!(json["status"], "pending");
}

#[test]
fn payment_bolt11_jit_roundtrip() {
	let p = Payment {
		id: HASH_32,
		kind: PaymentKind::Bolt11Jit(Bolt11Jit {
			hash: HASH_32_B,
			preimage: None,
			secret: Some(HASH_32_C),
			lsp_fee_limits: Some(LspFeeLimits {
				max_total_opening_fee_msat: Some(10_000),
				max_proportional_opening_fee_ppm_msat: Some(5000),
			}),
			counterparty_skimmed_fee_msat: Some(100),
		}),
		amount_msat: Some(100_000),
		fee_paid_msat: None,
		direction: PaymentDirection::Inbound,
		status: PaymentStatus::Pending,
		latest_update_timestamp: 1_700_000_000,
	};
	let json = roundtrip(&p);
	assert!(json["kind"]["bolt11_jit"].is_object());
}

#[test]
fn payment_bolt12_offer_roundtrip() {
	let p = Payment {
		id: HASH_32,
		kind: PaymentKind::Bolt12Offer(Bolt12Offer {
			hash: Some(HASH_32_B),
			preimage: None,
			secret: None,
			offer_id: HASH_32_C,
			payer_note: Some("thanks".into()),
			quantity: Some(2),
		}),
		amount_msat: Some(200_000),
		fee_paid_msat: Some(20),
		direction: PaymentDirection::Outbound,
		status: PaymentStatus::Succeeded,
		latest_update_timestamp: 1_700_000_000,
	};
	let json = roundtrip(&p);
	assert!(json["kind"]["bolt12_offer"].is_object());
	assert_eq!(json["kind"]["bolt12_offer"]["payer_note"], "thanks");
}

#[test]
fn payment_bolt12_refund_roundtrip() {
	let p = Payment {
		id: HASH_32,
		kind: PaymentKind::Bolt12Refund(Bolt12Refund {
			hash: None,
			preimage: None,
			secret: None,
			payer_note: None,
			quantity: None,
		}),
		amount_msat: None,
		fee_paid_msat: None,
		direction: PaymentDirection::Inbound,
		status: PaymentStatus::Failed,
		latest_update_timestamp: 1_700_000_000,
	};
	let json = roundtrip(&p);
	assert!(json["kind"]["bolt12_refund"].is_object());
	assert_eq!(json["status"], "failed");
}

#[test]
fn payment_spontaneous_roundtrip() {
	let p = Payment {
		id: HASH_32,
		kind: PaymentKind::Spontaneous(Spontaneous { hash: HASH_32_B, preimage: Some(HASH_32_C) }),
		amount_msat: Some(50_000),
		fee_paid_msat: None,
		direction: PaymentDirection::Inbound,
		status: PaymentStatus::Succeeded,
		latest_update_timestamp: 1_700_000_000,
	};
	let json = roundtrip(&p);
	assert!(json["kind"]["spontaneous"].is_object());
}

#[test]
fn forwarded_payment_roundtrip() {
	let fp = sample_forwarded_payment();
	let json = roundtrip(&fp);
	assert_eq!(json["prev_channel_id"], HASH_32_HEX);
	assert_eq!(json["prev_node_id"], PUBKEY_33_HEX);
	assert_eq!(json["claim_from_onchain_tx"], false);
}

#[test]
fn channel_roundtrip() {
	let ch = sample_channel();
	let json = roundtrip(&ch);
	assert_eq!(json["channel_id"], HASH_32_HEX);
	assert_eq!(json["counterparty_node_id"], PUBKEY_33_HEX);
	assert_eq!(json["is_usable"], true);
	assert!(json["funding_txo"].is_object());
	assert!(json["channel_config"].is_object());
}

#[test]
fn channel_config_roundtrip() {
	let cfg = sample_channel_config();
	let json = roundtrip(&cfg);
	assert!(json["max_dust_htlc_exposure"]["fixed_limit_msat"].is_number());
}

#[test]
fn max_dust_htlc_exposure_variants() {
	let fixed = MaxDustHtlcExposure::FixedLimitMsat(5_000_000);
	let json = roundtrip(&fixed);
	assert_eq!(json["fixed_limit_msat"], 5_000_000);

	let rate = MaxDustHtlcExposure::FeeRateMultiplier(1000);
	let json = roundtrip(&rate);
	assert_eq!(json["fee_rate_multiplier"], 1000);
}

#[test]
fn outpoint_roundtrip() {
	let op = OutPoint { txid: HASH_32, vout: 1 };
	let json = roundtrip(&op);
	assert_eq!(json["txid"], HASH_32_HEX);
	assert_eq!(json["vout"], 1);
}

#[test]
fn best_block_roundtrip() {
	let bb = BestBlock { block_hash: HASH_32, height: 800_000 };
	let json = roundtrip(&bb);
	assert_eq!(json["block_hash"], HASH_32_HEX);
	assert_eq!(json["height"], 800_000);
}

#[test]
fn confirmation_status_variants() {
	let confirmed = ConfirmationStatus::Confirmed(Confirmed {
		block_hash: HASH_32,
		height: 800_000,
		timestamp: 1_700_000_000,
	});
	let json = roundtrip(&confirmed);
	assert!(json["confirmed"].is_object());

	let unconfirmed = ConfirmationStatus::Unconfirmed(Unconfirmed {});
	let json = roundtrip(&unconfirmed);
	assert!(json["unconfirmed"].is_object());
}

#[test]
fn lightning_balance_claimable_on_channel_close() {
	let bal = LightningBalance::ClaimableOnChannelClose(ClaimableOnChannelClose {
		channel_id: HASH_32,
		counterparty_node_id: PUBKEY_33,
		amount_satoshis: 500_000,
		transaction_fee_satoshis: 300,
		outbound_payment_htlc_rounded_msat: 100,
		outbound_forwarded_htlc_rounded_msat: 200,
		inbound_claiming_htlc_rounded_msat: 50,
		inbound_htlc_rounded_msat: 25,
	});
	let json = roundtrip(&bal);
	assert!(json["claimable_on_channel_close"].is_object());
	assert_eq!(json["claimable_on_channel_close"]["channel_id"], HASH_32_HEX);
}

#[test]
fn lightning_balance_claimable_awaiting_confirmations() {
	let bal = LightningBalance::ClaimableAwaitingConfirmations(ClaimableAwaitingConfirmations {
		channel_id: HASH_32,
		counterparty_node_id: PUBKEY_33,
		amount_satoshis: 100_000,
		confirmation_height: 800_100,
		source: BalanceSource::HolderForceClosed,
	});
	let json = roundtrip(&bal);
	assert!(json["claimable_awaiting_confirmations"].is_object());
	assert_eq!(json["claimable_awaiting_confirmations"]["source"], "holder_force_closed");
}

#[test]
fn lightning_balance_contentious_claimable() {
	let bal = LightningBalance::ContentiousClaimable(ContentiousClaimable {
		channel_id: HASH_32,
		counterparty_node_id: PUBKEY_33,
		amount_satoshis: 50_000,
		timeout_height: 800_200,
		payment_hash: HASH_32_B,
		payment_preimage: HASH_32_C,
	});
	let json = roundtrip(&bal);
	assert!(json["contentious_claimable"].is_object());
}

#[test]
fn lightning_balance_maybe_timeout_claimable() {
	let bal = LightningBalance::MaybeTimeoutClaimableHtlc(MaybeTimeoutClaimableHtlc {
		channel_id: HASH_32,
		counterparty_node_id: PUBKEY_33,
		amount_satoshis: 25_000,
		claimable_height: 800_300,
		payment_hash: HASH_32_B,
		outbound_payment: true,
	});
	let json = roundtrip(&bal);
	assert!(json["maybe_timeout_claimable_htlc"].is_object());
}

#[test]
fn lightning_balance_maybe_preimage_claimable() {
	let bal = LightningBalance::MaybePreimageClaimableHtlc(MaybePreimageClaimableHtlc {
		channel_id: HASH_32,
		counterparty_node_id: PUBKEY_33,
		amount_satoshis: 10_000,
		expiry_height: 800_400,
		payment_hash: HASH_32_B,
	});
	let json = roundtrip(&bal);
	assert!(json["maybe_preimage_claimable_htlc"].is_object());
}

#[test]
fn lightning_balance_counterparty_revoked() {
	let bal =
		LightningBalance::CounterpartyRevokedOutputClaimable(CounterpartyRevokedOutputClaimable {
			channel_id: HASH_32,
			counterparty_node_id: PUBKEY_33,
			amount_satoshis: 75_000,
		});
	let json = roundtrip(&bal);
	assert!(json["counterparty_revoked_output_claimable"].is_object());
}

#[test]
fn balance_source_variants() {
	for (variant, expected) in [
		(BalanceSource::HolderForceClosed, "holder_force_closed"),
		(BalanceSource::CounterpartyForceClosed, "counterparty_force_closed"),
		(BalanceSource::CoopClose, "coop_close"),
		(BalanceSource::Htlc, "htlc"),
	] {
		let json = serde_json::to_value(&variant).unwrap();
		assert_eq!(json, expected);
		let back: BalanceSource = serde_json::from_value(json).unwrap();
		assert_eq!(back, variant);
	}
}

#[test]
fn pending_sweep_balance_pending_broadcast() {
	let bal = PendingSweepBalance::PendingBroadcast(PendingBroadcast {
		channel_id: Some(HASH_32),
		amount_satoshis: 50_000,
	});
	let json = roundtrip(&bal);
	assert!(json["pending_broadcast"].is_object());
	assert_eq!(json["pending_broadcast"]["channel_id"], HASH_32_HEX);
}

#[test]
fn pending_sweep_balance_pending_broadcast_no_channel() {
	let bal = PendingSweepBalance::PendingBroadcast(PendingBroadcast {
		channel_id: None,
		amount_satoshis: 50_000,
	});
	let json = roundtrip(&bal);
	assert!(json["pending_broadcast"]["channel_id"].is_null());
}

#[test]
fn pending_sweep_balance_broadcast_awaiting_confirmation() {
	let bal = PendingSweepBalance::BroadcastAwaitingConfirmation(BroadcastAwaitingConfirmation {
		channel_id: Some(HASH_32),
		latest_broadcast_height: 800_000,
		latest_spending_txid: HASH_32_B,
		amount_satoshis: 50_000,
	});
	let json = roundtrip(&bal);
	assert!(json["broadcast_awaiting_confirmation"].is_object());
}

#[test]
fn pending_sweep_balance_awaiting_threshold() {
	let bal = PendingSweepBalance::AwaitingThresholdConfirmations(AwaitingThresholdConfirmations {
		channel_id: None,
		latest_spending_txid: HASH_32,
		confirmation_hash: HASH_32_B,
		confirmation_height: 800_010,
		amount_satoshis: 50_000,
	});
	let json = roundtrip(&bal);
	assert!(json["awaiting_threshold_confirmations"].is_object());
}

#[test]
fn bolt11_invoice_description_variants() {
	let direct = Bolt11InvoiceDescription::Direct("coffee".into());
	let json = roundtrip(&direct);
	assert_eq!(json["direct"], "coffee");

	let hash = Bolt11InvoiceDescription::Hash("abc123".into());
	let json = roundtrip(&hash);
	assert_eq!(json["hash"], "abc123");
}

#[test]
fn route_parameters_config_roundtrip() {
	let rp = sample_route_params();
	let json = roundtrip(&rp);
	assert_eq!(json["max_total_cltv_expiry_delta"], 1008);
	assert_eq!(json["max_path_count"], 10);
}

#[test]
fn graph_routing_fees_roundtrip() {
	let fees = GraphRoutingFees { base_msat: 1000, proportional_millionths: 100 };
	roundtrip(&fees);
}

#[test]
fn graph_channel_update_roundtrip() {
	let update = GraphChannelUpdate {
		last_update: 1_700_000_000,
		enabled: true,
		cltv_expiry_delta: 144,
		htlc_minimum_msat: 1000,
		htlc_maximum_msat: 1_000_000_000,
		fees: GraphRoutingFees { base_msat: 1000, proportional_millionths: 100 },
	};
	roundtrip(&update);
}

#[test]
fn graph_channel_roundtrip() {
	let ch = GraphChannel {
		node_one: PUBKEY_33,
		node_two: PUBKEY_33_B,
		capacity_sats: Some(1_000_000),
		one_to_two: Some(GraphChannelUpdate {
			last_update: 1_700_000_000,
			enabled: true,
			cltv_expiry_delta: 144,
			htlc_minimum_msat: 1000,
			htlc_maximum_msat: 1_000_000_000,
			fees: GraphRoutingFees { base_msat: 1000, proportional_millionths: 100 },
		}),
		two_to_one: None,
	};
	let json = roundtrip(&ch);
	assert_eq!(json["node_one"], PUBKEY_33_HEX);
}

#[test]
fn graph_node_announcement_roundtrip() {
	let ann = GraphNodeAnnouncement {
		last_update: 1_700_000_000,
		alias: "my-node".into(),
		rgb: "ff6600".into(),
		addresses: vec!["127.0.0.1:9735".into()],
	};
	roundtrip(&ann);
}

#[test]
fn graph_node_roundtrip() {
	let node = GraphNode {
		channels: vec![123456789, 987654321],
		announcement_info: Some(GraphNodeAnnouncement {
			last_update: 1_700_000_000,
			alias: "test-node".into(),
			rgb: "aabbcc".into(),
			addresses: vec![],
		}),
	};
	roundtrip(&node);
}

#[test]
fn peer_roundtrip() {
	let peer = Peer {
		node_id: PUBKEY_33,
		address: "127.0.0.1:9735".into(),
		is_persisted: true,
		is_connected: true,
	};
	let json = roundtrip(&peer);
	assert_eq!(json["node_id"], PUBKEY_33_HEX);
}

#[test]
fn payment_direction_variants() {
	for (variant, expected) in
		[(PaymentDirection::Inbound, "inbound"), (PaymentDirection::Outbound, "outbound")]
	{
		let json = serde_json::to_value(&variant).unwrap();
		assert_eq!(json, expected);
		let back: PaymentDirection = serde_json::from_value(json).unwrap();
		assert_eq!(back, variant);
	}
}

#[test]
fn payment_status_variants() {
	for (variant, expected) in [
		(PaymentStatus::Pending, "pending"),
		(PaymentStatus::Succeeded, "succeeded"),
		(PaymentStatus::Failed, "failed"),
	] {
		let json = serde_json::to_value(&variant).unwrap();
		assert_eq!(json, expected);
		let back: PaymentStatus = serde_json::from_value(json).unwrap();
		assert_eq!(back, variant);
	}
}

#[test]
fn lsp_fee_limits_roundtrip() {
	let limits = LspFeeLimits {
		max_total_opening_fee_msat: Some(10_000),
		max_proportional_opening_fee_ppm_msat: None,
	};
	roundtrip(&limits);
}

// ===========================================================================
// events.rs
// ===========================================================================

#[test]
fn event_payment_received() {
	let ev = Event::PaymentReceived(PaymentReceived { payment: sample_payment() });
	let json = roundtrip(&ev);
	assert!(json["payment_received"].is_object());
	assert!(json["payment_received"]["payment"].is_object());
}

#[test]
fn event_payment_successful() {
	let ev = Event::PaymentSuccessful(PaymentSuccessful { payment: sample_payment() });
	let json = roundtrip(&ev);
	assert!(json["payment_successful"].is_object());
}

#[test]
fn event_payment_failed() {
	let ev = Event::PaymentFailed(PaymentFailed { payment: sample_payment() });
	let json = roundtrip(&ev);
	assert!(json["payment_failed"].is_object());
}

#[test]
fn event_payment_claimable() {
	let ev = Event::PaymentClaimable(PaymentClaimable { payment: sample_payment() });
	let json = roundtrip(&ev);
	assert!(json["payment_claimable"].is_object());
}

#[test]
fn event_payment_forwarded() {
	let ev =
		Event::PaymentForwarded(PaymentForwarded { forwarded_payment: sample_forwarded_payment() });
	let json = roundtrip(&ev);
	assert!(json["payment_forwarded"].is_object());
	assert!(json["payment_forwarded"]["forwarded_payment"].is_object());
}

// ===========================================================================
// api.rs
// ===========================================================================

#[test]
fn get_node_info_roundtrip() {
	roundtrip(&GetNodeInfoRequest {});
	let resp = GetNodeInfoResponse {
		node_id: PUBKEY_33,
		current_best_block: BestBlock { block_hash: HASH_32, height: 800_000 },
		latest_lightning_wallet_sync_timestamp: Some(1_700_000_000),
		latest_onchain_wallet_sync_timestamp: Some(1_700_000_000),
		latest_fee_rate_cache_update_timestamp: None,
		latest_rgs_snapshot_timestamp: None,
		latest_node_announcement_broadcast_timestamp: None,
		listening_addresses: vec!["0.0.0.0:9735".into()],
		announcement_addresses: vec![],
		node_alias: Some("my-node".into()),
		node_uris: vec![],
	};
	let json = roundtrip(&resp);
	assert_eq!(json["node_id"], PUBKEY_33_HEX);
}

#[test]
fn onchain_receive_roundtrip() {
	roundtrip(&OnchainReceiveRequest {});
	roundtrip(&OnchainReceiveResponse { address: "bc1qtest".into() });
}

#[test]
fn onchain_send_roundtrip() {
	let req = OnchainSendRequest {
		address: "bc1qtest".into(),
		amount_sats: Some(100_000),
		send_all: None,
		fee_rate_sat_per_vb: Some(5),
	};
	roundtrip(&req);

	let resp = OnchainSendResponse { txid: HASH_32, payment_id: HASH_32_B };
	let json = roundtrip(&resp);
	assert_eq!(json["txid"], HASH_32_HEX);
	assert_eq!(json["payment_id"], HASH_32_B_HEX);
}

#[test]
fn onchain_send_all_roundtrip() {
	let req = OnchainSendRequest {
		address: "bc1qtest".into(),
		amount_sats: None,
		send_all: Some(true),
		fee_rate_sat_per_vb: None,
	};
	roundtrip(&req);
}

#[test]
fn bolt11_receive_roundtrip() {
	let req = Bolt11ReceiveRequest {
		amount_msat: Some(100_000),
		description: Some(Bolt11InvoiceDescription::Direct("test".into())),
		expiry_secs: 3600,
	};
	roundtrip(&req);

	let resp = Bolt11ReceiveResponse {
		invoice: "lnbc1...".into(),
		payment_hash: HASH_32,
		payment_secret: HASH_32_B,
	};
	let json = roundtrip(&resp);
	assert_eq!(json["payment_hash"], HASH_32_HEX);
}

#[test]
fn bolt11_receive_variable_amount_roundtrip() {
	let req = Bolt11ReceiveRequest { amount_msat: None, description: None, expiry_secs: 3600 };
	roundtrip(&req);
}

#[test]
fn bolt11_receive_for_hash_roundtrip() {
	let req = Bolt11ReceiveForHashRequest {
		amount_msat: Some(50_000),
		description: Some(Bolt11InvoiceDescription::Hash("deadbeef".into())),
		expiry_secs: 1800,
		payment_hash: HASH_32,
	};
	let json = roundtrip(&req);
	assert_eq!(json["payment_hash"], HASH_32_HEX);
}

#[test]
fn bolt11_claim_for_hash_roundtrip() {
	let req = Bolt11ClaimForHashRequest {
		payment_hash: Some(HASH_32),
		claimable_amount_msat: Some(100_000),
		preimage: HASH_32_B,
	};
	roundtrip(&req);
	roundtrip(&Bolt11ClaimForHashResponse {});
}

#[test]
fn bolt11_fail_for_hash_roundtrip() {
	let req = Bolt11FailForHashRequest { payment_hash: HASH_32 };
	let json = roundtrip(&req);
	assert_eq!(json["payment_hash"], HASH_32_HEX);
	roundtrip(&Bolt11FailForHashResponse {});
}

#[test]
fn bolt11_receive_via_jit_channel_roundtrip() {
	let req = Bolt11ReceiveViaJitChannelRequest {
		amount_msat: 1_000_000,
		description: Some(Bolt11InvoiceDescription::Direct("jit test".into())),
		expiry_secs: 3600,
		max_total_lsp_fee_limit_msat: Some(5000),
	};
	roundtrip(&req);
}

#[test]
fn bolt11_receive_variable_amount_via_jit_channel_roundtrip() {
	let req = Bolt11ReceiveVariableAmountViaJitChannelRequest {
		description: None,
		expiry_secs: 3600,
		max_proportional_lsp_fee_limit_ppm_msat: Some(1000),
	};
	roundtrip(&req);
}

#[test]
fn bolt11_send_roundtrip() {
	let req = Bolt11SendRequest {
		invoice: "lnbc1...".into(),
		amount_msat: Some(100_000),
		route_parameters: Some(sample_route_params()),
	};
	roundtrip(&req);

	let resp = Bolt11SendResponse { payment_id: HASH_32 };
	let json = roundtrip(&resp);
	assert_eq!(json["payment_id"], HASH_32_HEX);
}

#[test]
fn bolt12_receive_roundtrip() {
	let req = Bolt12ReceiveRequest {
		description: "test offer".into(),
		amount_msat: Some(500_000),
		expiry_secs: Some(86400),
		quantity: Some(1),
	};
	roundtrip(&req);

	let resp = Bolt12ReceiveResponse { offer: "lno1...".into(), offer_id: HASH_32 };
	let json = roundtrip(&resp);
	assert_eq!(json["offer_id"], HASH_32_HEX);
}

#[test]
fn bolt12_send_roundtrip() {
	let req = Bolt12SendRequest {
		offer: "lno1...".into(),
		amount_msat: Some(500_000),
		quantity: Some(1),
		payer_note: Some("thanks".into()),
		route_parameters: None,
	};
	roundtrip(&req);

	let resp = Bolt12SendResponse { payment_id: HASH_32 };
	roundtrip(&resp);
}

#[test]
fn spontaneous_send_roundtrip() {
	let req = SpontaneousSendRequest {
		amount_msat: 100_000,
		node_id: PUBKEY_33,
		route_parameters: Some(sample_route_params()),
	};
	let json = roundtrip(&req);
	assert_eq!(json["node_id"], PUBKEY_33_HEX);

	let resp = SpontaneousSendResponse { payment_id: HASH_32 };
	roundtrip(&resp);
}

#[test]
fn open_channel_roundtrip() {
	let req = OpenChannelRequest {
		node_pubkey: PUBKEY_33,
		address: "127.0.0.1:9735".into(),
		channel_amount_sats: 1_000_000,
		push_to_counterparty_msat: Some(10_000),
		channel_config: Some(sample_channel_config()),
		announce_channel: true,
	};
	let json = roundtrip(&req);
	assert_eq!(json["node_pubkey"], PUBKEY_33_HEX);

	roundtrip(&OpenChannelResponse { user_channel_id: "abc123".into() });
}

#[test]
fn splice_in_roundtrip() {
	let req = SpliceInRequest {
		user_channel_id: "abc123".into(),
		counterparty_node_id: PUBKEY_33,
		splice_amount_sats: 500_000,
	};
	roundtrip(&req);
	roundtrip(&SpliceInResponse {});
}

#[test]
fn splice_out_roundtrip() {
	let req = SpliceOutRequest {
		user_channel_id: "abc123".into(),
		counterparty_node_id: PUBKEY_33,
		address: Some("bc1qtest".into()),
		splice_amount_sats: 200_000,
	};
	roundtrip(&req);
	roundtrip(&SpliceOutResponse { address: "bc1qtest".into() });
}

#[test]
fn update_channel_config_roundtrip() {
	let req = UpdateChannelConfigRequest {
		user_channel_id: "abc123".into(),
		counterparty_node_id: PUBKEY_33,
		channel_config: Some(sample_channel_config()),
	};
	roundtrip(&req);
	roundtrip(&UpdateChannelConfigResponse {});
}

#[test]
fn close_channel_roundtrip() {
	let req =
		CloseChannelRequest { user_channel_id: "abc123".into(), counterparty_node_id: PUBKEY_33 };
	let json = roundtrip(&req);
	assert_eq!(json["counterparty_node_id"], PUBKEY_33_HEX);
	roundtrip(&CloseChannelResponse {});
}

#[test]
fn force_close_channel_roundtrip() {
	let req = ForceCloseChannelRequest {
		user_channel_id: "abc123".into(),
		counterparty_node_id: PUBKEY_33,
		force_close_reason: Some("unresponsive".into()),
	};
	roundtrip(&req);
	roundtrip(&ForceCloseChannelResponse {});
}

#[test]
fn list_channels_roundtrip() {
	roundtrip(&ListChannelsRequest {});
	let resp = ListChannelsResponse { channels: vec![sample_channel()] };
	roundtrip(&resp);
}

#[test]
fn get_payment_details_roundtrip() {
	let req = GetPaymentDetailsRequest { payment_id: HASH_32 };
	let json = roundtrip(&req);
	assert_eq!(json["payment_id"], HASH_32_HEX);

	let resp_some = GetPaymentDetailsResponse { payment: Some(sample_payment()) };
	roundtrip(&resp_some);

	let resp_none = GetPaymentDetailsResponse { payment: None };
	roundtrip(&resp_none);
}

#[test]
fn list_payments_roundtrip() {
	let req = ListPaymentsRequest { page_token: Some("token123".into()) };
	roundtrip(&req);

	let resp = ListPaymentsResponse {
		payments: vec![sample_payment()],
		next_page_token: Some("next_token".into()),
	};
	roundtrip(&resp);
}

#[test]
fn list_forwarded_payments_roundtrip() {
	let req = ListForwardedPaymentsRequest { page_token: None };
	roundtrip(&req);

	let resp = ListForwardedPaymentsResponse {
		forwarded_payments: vec![sample_forwarded_payment()],
		next_page_token: None,
	};
	roundtrip(&resp);
}

#[test]
fn sign_message_roundtrip() {
	let req = SignMessageRequest { message: vec![0x48, 0x65, 0x6c, 0x6c, 0x6f] };
	let json = roundtrip(&req);
	assert_eq!(json["message"], "48656c6c6f"); // "Hello" in hex

	let resp = SignMessageResponse { signature: "d2mea3...".into() };
	roundtrip(&resp);
}

#[test]
fn verify_signature_roundtrip() {
	let req = VerifySignatureRequest {
		message: vec![0x48, 0x69],
		signature: "d2mea3...".into(),
		public_key: PUBKEY_33,
	};
	let json = roundtrip(&req);
	assert_eq!(json["public_key"], PUBKEY_33_HEX);

	let resp = VerifySignatureResponse { valid: true };
	let json = roundtrip(&resp);
	assert_eq!(json["valid"], true);
}

#[test]
fn export_pathfinding_scores_roundtrip() {
	roundtrip(&ExportPathfindingScoresRequest {});

	let resp = ExportPathfindingScoresResponse { scores: vec![0xde, 0xad, 0xbe, 0xef] };
	let json = roundtrip(&resp);
	assert_eq!(json["scores"], "deadbeef");
}

#[test]
fn get_balances_roundtrip() {
	roundtrip(&GetBalancesRequest {});

	let resp = GetBalancesResponse {
		total_onchain_balance_sats: 1_000_000,
		spendable_onchain_balance_sats: 900_000,
		total_anchor_channels_reserve_sats: 100_000,
		total_lightning_balance_sats: 500_000,
		lightning_balances: vec![LightningBalance::ClaimableOnChannelClose(
			ClaimableOnChannelClose {
				channel_id: HASH_32,
				counterparty_node_id: PUBKEY_33,
				amount_satoshis: 500_000,
				transaction_fee_satoshis: 300,
				outbound_payment_htlc_rounded_msat: 0,
				outbound_forwarded_htlc_rounded_msat: 0,
				inbound_claiming_htlc_rounded_msat: 0,
				inbound_htlc_rounded_msat: 0,
			},
		)],
		pending_balances_from_channel_closures: vec![PendingSweepBalance::PendingBroadcast(
			PendingBroadcast { channel_id: Some(HASH_32), amount_satoshis: 50_000 },
		)],
	};
	roundtrip(&resp);
}

#[test]
fn connect_peer_roundtrip() {
	let req = ConnectPeerRequest {
		node_pubkey: PUBKEY_33,
		address: "127.0.0.1:9735".into(),
		persist: true,
	};
	let json = roundtrip(&req);
	assert_eq!(json["node_pubkey"], PUBKEY_33_HEX);
	roundtrip(&ConnectPeerResponse {});
}

#[test]
fn disconnect_peer_roundtrip() {
	let req = DisconnectPeerRequest { node_pubkey: PUBKEY_33 };
	let json = roundtrip(&req);
	assert_eq!(json["node_pubkey"], PUBKEY_33_HEX);
	roundtrip(&DisconnectPeerResponse {});
}

#[test]
fn list_peers_roundtrip() {
	roundtrip(&ListPeersRequest {});

	let resp = ListPeersResponse {
		peers: vec![Peer {
			node_id: PUBKEY_33,
			address: "127.0.0.1:9735".into(),
			is_persisted: true,
			is_connected: false,
		}],
	};
	roundtrip(&resp);
}

#[test]
fn graph_list_channels_roundtrip() {
	roundtrip(&GraphListChannelsRequest {});
	roundtrip(&GraphListChannelsResponse { short_channel_ids: vec![123456789, 987654321] });
}

#[test]
fn graph_get_channel_roundtrip() {
	let req = GraphGetChannelRequest { short_channel_id: 123456789 };
	roundtrip(&req);

	let resp = GraphGetChannelResponse {
		channel: Some(GraphChannel {
			node_one: PUBKEY_33,
			node_two: PUBKEY_33_B,
			capacity_sats: Some(1_000_000),
			one_to_two: None,
			two_to_one: None,
		}),
	};
	roundtrip(&resp);

	roundtrip(&GraphGetChannelResponse { channel: None });
}

#[test]
fn graph_list_nodes_roundtrip() {
	roundtrip(&GraphListNodesRequest {});
	roundtrip(&GraphListNodesResponse { node_ids: vec!["abc".into(), "def".into()] });
}

#[test]
fn graph_get_node_roundtrip() {
	let req = GraphGetNodeRequest { node_id: PUBKEY_33 };
	let json = roundtrip(&req);
	assert_eq!(json["node_id"], PUBKEY_33_HEX);

	let resp = GraphGetNodeResponse {
		node: Some(GraphNode {
			channels: vec![123],
			announcement_info: Some(GraphNodeAnnouncement {
				last_update: 1_700_000_000,
				alias: "test".into(),
				rgb: "ffffff".into(),
				addresses: vec![],
			}),
		}),
	};
	roundtrip(&resp);

	roundtrip(&GraphGetNodeResponse { node: None });
}

#[test]
fn unified_send_request_roundtrip() {
	let req = UnifiedSendRequest {
		uri: "bitcoin:bc1q...?lightning=lnbc1...".into(),
		amount_msat: Some(100_000),
		route_parameters: Some(sample_route_params()),
	};
	roundtrip(&req);
}

#[test]
fn unified_send_response_onchain() {
	let resp = UnifiedSendResponse::Onchain { txid: HASH_32, payment_id: HASH_32_B };
	let json = roundtrip(&resp);
	assert!(json["onchain"].is_object());
	assert_eq!(json["onchain"]["txid"], HASH_32_HEX);
}

#[test]
fn unified_send_response_bolt11() {
	let resp = UnifiedSendResponse::Bolt11 { payment_id: HASH_32 };
	let json = roundtrip(&resp);
	assert!(json["bolt11"].is_object());
	assert_eq!(json["bolt11"]["payment_id"], HASH_32_HEX);
}

#[test]
fn unified_send_response_bolt12() {
	let resp = UnifiedSendResponse::Bolt12 { payment_id: HASH_32 };
	let json = roundtrip(&resp);
	assert!(json["bolt12"].is_object());
}

// ===========================================================================
// Cross-cutting: hex serde round-trips from raw JSON strings
// ===========================================================================

#[test]
fn hex_32_from_json_string() {
	let json_str = format!(r#"{{"payment_id":"{}"}}"#, HASH_32_HEX);
	let req: GetPaymentDetailsRequest = serde_json::from_str(&json_str).unwrap();
	assert_eq!(req.payment_id, HASH_32);
}

#[test]
fn hex_33_from_json_string() {
	let json_str = format!(
		r#"{{"node_pubkey":"{}","address":"127.0.0.1:9735","persist":true}}"#,
		PUBKEY_33_HEX
	);
	let req: ConnectPeerRequest = serde_json::from_str(&json_str).unwrap();
	assert_eq!(req.node_pubkey, PUBKEY_33);
}

#[test]
fn opt_hex_32_null_from_json() {
	let json_str = r#"{"hash":"1111111111111111111111111111111111111111111111111111111111111111","preimage":null,"secret":null}"#;
	let bolt11: Bolt11 = serde_json::from_str(json_str).unwrap();
	assert_eq!(bolt11.hash, HASH_32_B);
	assert_eq!(bolt11.preimage, None);
	assert_eq!(bolt11.secret, None);
}

#[test]
fn opt_hex_32_absent_from_json() {
	// Fields with #[serde(default)] should work when absent
	let json_str = r#"{"hash":"1111111111111111111111111111111111111111111111111111111111111111"}"#;
	let bolt11: Bolt11 = serde_json::from_str(json_str).unwrap();
	assert_eq!(bolt11.preimage, None);
	assert_eq!(bolt11.secret, None);
}

#[test]
fn bytes_hex_roundtrip() {
	let req = SignMessageRequest { message: vec![] };
	let json = roundtrip(&req);
	assert_eq!(json["message"], "");

	let req = SignMessageRequest { message: vec![0xff, 0x00, 0xab] };
	let json = roundtrip(&req);
	assert_eq!(json["message"], "ff00ab");
}
