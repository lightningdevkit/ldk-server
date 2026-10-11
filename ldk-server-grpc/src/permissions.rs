// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use crate::endpoints::{
	BOLT11_CLAIM_FOR_ID_PATH, BOLT11_FAIL_FOR_ID_PATH, BOLT11_RECEIVE_FOR_HASH_PATH,
	BOLT11_RECEIVE_PATH, BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_FOR_HASH_PATH,
	BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH,
	BOLT11_RECEIVE_VIA_JIT_CHANNEL_FOR_HASH_PATH, BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH,
	BOLT11_SEND_PATH, BOLT11_SEND_UNDERPAYING_PATH, BOLT12_CREATE_PAYER_PROOF_PATH,
	BOLT12_RECEIVE_PATH, BOLT12_RECEIVE_REFUND_PATH, BOLT12_SEND_PATH, BOLT12_SEND_REFUND_PATH,
	BUMP_CHANNEL_FUNDING_FEE_PATH, CLOSE_CHANNEL_PATH, CONNECT_PEER_PATH, CREATE_MACAROON_PATH,
	DECODE_INVOICE_PATH, DECODE_OFFER_PATH, DISCONNECT_PEER_PATH, EXPORT_PATHFINDING_SCORES_PATH,
	FORCE_CLOSE_CHANNEL_PATH, GET_BALANCES_PATH, GET_CHANNEL_FORWARDING_STATS_PATH,
	GET_FORWARDED_PAYMENT_DETAILS_PATH, GET_FORWARDED_PAYMENT_TRACKING_MODE_PATH,
	GET_NODE_INFO_PATH, GET_PAYMENT_DETAILS_PATH, GET_PERMISSIONS_PATH, GRAPH_GET_CHANNEL_PATH,
	GRAPH_GET_NODE_PATH, GRAPH_LIST_CHANNELS_PATH, GRAPH_LIST_NODES_PATH, LIST_CHANNELS_PATH,
	LIST_CHANNEL_FORWARDING_STATS_PATH, LIST_CHANNEL_PAIR_FORWARDING_STATS_PATH,
	LIST_FORWARDED_PAYMENTS_PATH, LIST_MACAROONS_PATH, LIST_PAYMENTS_PATH, LIST_PEERS_PATH,
	ONCHAIN_BUMP_FEE_PATH, ONCHAIN_RECEIVE_PATH, ONCHAIN_SEND_PATH, OPEN_CHANNEL_PATH,
	REVOKE_MACAROON_PATH, SIGN_MESSAGE_PATH, SPLICE_IN_PATH, SPLICE_OUT_PATH,
	SPONTANEOUS_SEND_PATH, SUBSCRIBE_EVENTS_PATH, UNIFIED_SEND_PATH, UPDATE_CHANNEL_CONFIG_PATH,
	VERIFY_SIGNATURE_PATH,
};

pub const ADMIN_PERMISSION: &str = "admin";
pub const NODE_READ_PERMISSION: &str = "node:read";
pub const ONCHAIN_RECEIVE_PERMISSION: &str = "onchain:receive";
pub const ONCHAIN_SEND_PERMISSION: &str = "onchain:send";
pub const INVOICES_CREATE_PERMISSION: &str = "invoices:create";
pub const PAYMENTS_READ_PERMISSION: &str = "payments:read";
pub const PAYMENTS_CLAIM_PERMISSION: &str = "payments:claim";
pub const PAYMENTS_SEND_PERMISSION: &str = "payments:send";
pub const CHANNELS_READ_PERMISSION: &str = "channels:read";
pub const CHANNELS_MANAGE_PERMISSION: &str = "channels:manage";
pub const CHANNELS_FORCE_CLOSE_PERMISSION: &str = "channels:force_close";
pub const PEERS_READ_PERMISSION: &str = "peers:read";
pub const PEERS_MANAGE_PERMISSION: &str = "peers:manage";
pub const MESSAGES_SIGN_PERMISSION: &str = "messages:sign";
pub const MESSAGES_VERIFY_PERMISSION: &str = "messages:verify";
pub const GRAPH_READ_PERMISSION: &str = "graph:read";
pub const UTILITIES_READ_PERMISSION: &str = "utilities:read";
pub const EVENTS_READ_PERMISSION: &str = "events:read";
pub const MACAROONS_MANAGE_PERMISSION: &str = "macaroons:manage";

/// All permissions accepted when a macaroon is created.
pub const ALL_PERMISSIONS: [&str; 19] = [
	ADMIN_PERMISSION,
	NODE_READ_PERMISSION,
	ONCHAIN_RECEIVE_PERMISSION,
	ONCHAIN_SEND_PERMISSION,
	INVOICES_CREATE_PERMISSION,
	PAYMENTS_READ_PERMISSION,
	PAYMENTS_CLAIM_PERMISSION,
	PAYMENTS_SEND_PERMISSION,
	CHANNELS_READ_PERMISSION,
	CHANNELS_MANAGE_PERMISSION,
	CHANNELS_FORCE_CLOSE_PERMISSION,
	PEERS_READ_PERMISSION,
	PEERS_MANAGE_PERMISSION,
	MESSAGES_SIGN_PERMISSION,
	MESSAGES_VERIFY_PERMISSION,
	GRAPH_READ_PERMISSION,
	UTILITIES_READ_PERMISSION,
	EVENTS_READ_PERMISSION,
	MACAROONS_MANAGE_PERMISSION,
];

/// Permissions included in the CLI `readonly` preset.
pub const READONLY_PERMISSIONS: [&str; 8] = [
	NODE_READ_PERMISSION,
	PAYMENTS_READ_PERMISSION,
	CHANNELS_READ_PERMISSION,
	PEERS_READ_PERMISSION,
	MESSAGES_VERIFY_PERMISSION,
	GRAPH_READ_PERMISSION,
	UTILITIES_READ_PERMISSION,
	EVENTS_READ_PERMISSION,
];

/// Permissions included in the CLI `invoice` preset.
pub const INVOICE_PERMISSIONS: [&str; 11] = [
	NODE_READ_PERMISSION,
	ONCHAIN_RECEIVE_PERMISSION,
	INVOICES_CREATE_PERMISSION,
	PAYMENTS_READ_PERMISSION,
	PAYMENTS_CLAIM_PERMISSION,
	CHANNELS_READ_PERMISSION,
	PEERS_READ_PERMISSION,
	MESSAGES_VERIFY_PERMISSION,
	GRAPH_READ_PERMISSION,
	UTILITIES_READ_PERMISSION,
	EVENTS_READ_PERMISSION,
];

/// Named permission sets for issuing macaroons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacaroonPreset {
	Readonly,
	Invoice,
	Admin,
}

impl MacaroonPreset {
	/// All supported presets, in display order.
	pub const ALL: [Self; 3] = [Self::Readonly, Self::Invoice, Self::Admin];

	/// The lowercase name of this preset.
	pub fn name(self) -> &'static str {
		match self {
			Self::Readonly => "readonly",
			Self::Invoice => "invoice",
			Self::Admin => "admin",
		}
	}

	/// Permissions granted by this preset.
	pub fn permissions(self) -> Vec<String> {
		match self {
			Self::Readonly => {
				READONLY_PERMISSIONS.iter().map(|value| (*value).to_string()).collect()
			},
			Self::Invoice => INVOICE_PERMISSIONS.iter().map(|value| (*value).to_string()).collect(),
			Self::Admin => vec![ADMIN_PERMISSION.to_string()],
		}
	}
}

impl std::str::FromStr for MacaroonPreset {
	type Err = &'static str;

	fn from_str(name: &str) -> Result<Self, Self::Err> {
		Self::ALL.into_iter().find(|preset| preset.name() == name).ok_or("Unknown macaroon preset")
	}
}

/// How an RPC method is authorized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MethodAuthorization {
	/// The caller needs this permission, or `admin`.
	Permission(&'static str),
	/// Any authenticated caller may use the method.
	AuthenticatedOnly,
	/// The method is not known to the server and is rejected.
	Unknown,
}

/// Returns how `method`, an RPC name such as [`GET_NODE_INFO_PATH`], is authorized.
///
/// This is the mapping the server enforces, so clients can use it to tell which RPCs a
/// macaroon's permissions allow.
pub fn method_authorization(method: &str) -> MethodAuthorization {
	match method {
		GET_NODE_INFO_PATH | GET_BALANCES_PATH | EXPORT_PATHFINDING_SCORES_PATH => {
			MethodAuthorization::Permission(NODE_READ_PERMISSION)
		},
		ONCHAIN_RECEIVE_PATH => MethodAuthorization::Permission(ONCHAIN_RECEIVE_PERMISSION),
		ONCHAIN_SEND_PATH | ONCHAIN_BUMP_FEE_PATH => {
			MethodAuthorization::Permission(ONCHAIN_SEND_PERMISSION)
		},
		BOLT11_RECEIVE_PATH
		| BOLT11_RECEIVE_FOR_HASH_PATH
		| BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH
		| BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH
		| BOLT11_RECEIVE_VIA_JIT_CHANNEL_FOR_HASH_PATH
		| BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_FOR_HASH_PATH
		| BOLT12_RECEIVE_PATH
		| BOLT12_RECEIVE_REFUND_PATH => MethodAuthorization::Permission(INVOICES_CREATE_PERMISSION),
		BOLT11_CLAIM_FOR_ID_PATH | BOLT11_FAIL_FOR_ID_PATH => {
			MethodAuthorization::Permission(PAYMENTS_CLAIM_PERMISSION)
		},
		BOLT11_SEND_PATH
		| BOLT11_SEND_UNDERPAYING_PATH
		| BOLT12_SEND_PATH
		| BOLT12_SEND_REFUND_PATH
		| SPONTANEOUS_SEND_PATH
		| UNIFIED_SEND_PATH
		| SPLICE_OUT_PATH => MethodAuthorization::Permission(PAYMENTS_SEND_PERMISSION),
		GET_PAYMENT_DETAILS_PATH
		| LIST_PAYMENTS_PATH
		| LIST_FORWARDED_PAYMENTS_PATH
		| GET_FORWARDED_PAYMENT_DETAILS_PATH
		| GET_FORWARDED_PAYMENT_TRACKING_MODE_PATH
		| GET_CHANNEL_FORWARDING_STATS_PATH
		| LIST_CHANNEL_FORWARDING_STATS_PATH
		| LIST_CHANNEL_PAIR_FORWARDING_STATS_PATH => {
			MethodAuthorization::Permission(PAYMENTS_READ_PERMISSION)
		},
		LIST_CHANNELS_PATH => MethodAuthorization::Permission(CHANNELS_READ_PERMISSION),
		OPEN_CHANNEL_PATH
		| UPDATE_CHANNEL_CONFIG_PATH
		| CLOSE_CHANNEL_PATH
		| SPLICE_IN_PATH
		| BUMP_CHANNEL_FUNDING_FEE_PATH => MethodAuthorization::Permission(CHANNELS_MANAGE_PERMISSION),
		FORCE_CLOSE_CHANNEL_PATH => {
			MethodAuthorization::Permission(CHANNELS_FORCE_CLOSE_PERMISSION)
		},
		LIST_PEERS_PATH => MethodAuthorization::Permission(PEERS_READ_PERMISSION),
		CONNECT_PEER_PATH | DISCONNECT_PEER_PATH => {
			MethodAuthorization::Permission(PEERS_MANAGE_PERMISSION)
		},
		SIGN_MESSAGE_PATH | BOLT12_CREATE_PAYER_PROOF_PATH => {
			MethodAuthorization::Permission(MESSAGES_SIGN_PERMISSION)
		},
		VERIFY_SIGNATURE_PATH => MethodAuthorization::Permission(MESSAGES_VERIFY_PERMISSION),
		GRAPH_LIST_CHANNELS_PATH
		| GRAPH_GET_CHANNEL_PATH
		| GRAPH_LIST_NODES_PATH
		| GRAPH_GET_NODE_PATH => MethodAuthorization::Permission(GRAPH_READ_PERMISSION),
		DECODE_INVOICE_PATH | DECODE_OFFER_PATH => {
			MethodAuthorization::Permission(UTILITIES_READ_PERMISSION)
		},
		SUBSCRIBE_EVENTS_PATH => MethodAuthorization::Permission(EVENTS_READ_PERMISSION),
		CREATE_MACAROON_PATH | LIST_MACAROONS_PATH | REVOKE_MACAROON_PATH => {
			MethodAuthorization::Permission(MACAROONS_MANAGE_PERMISSION)
		},
		GET_PERMISSIONS_PATH => MethodAuthorization::AuthenticatedOnly,
		_ => MethodAuthorization::Unknown,
	}
}
