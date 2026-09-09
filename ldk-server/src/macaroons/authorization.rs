// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! RPC permission requirements.

use ldk_server_grpc::endpoints::{
	BOLT11_CLAIM_FOR_ID_PATH, BOLT11_FAIL_FOR_ID_PATH, BOLT11_RECEIVE_FOR_HASH_PATH,
	BOLT11_RECEIVE_PATH, BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH,
	BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH, BOLT11_SEND_PATH, BOLT11_SEND_UNDERPAYING_PATH,
	BOLT12_CREATE_PAYER_PROOF_PATH, BOLT12_RECEIVE_PATH, BOLT12_RECEIVE_REFUND_PATH,
	BOLT12_SEND_PATH, BOLT12_SEND_REFUND_PATH, CLOSE_CHANNEL_PATH, CONNECT_PEER_PATH,
	CREATE_MACAROON_PATH, DECODE_INVOICE_PATH, DECODE_OFFER_PATH, DISCONNECT_PEER_PATH,
	EXPORT_PATHFINDING_SCORES_PATH, FORCE_CLOSE_CHANNEL_PATH, GET_BALANCES_PATH,
	GET_NODE_INFO_PATH, GET_PAYMENT_DETAILS_PATH, GET_PERMISSIONS_PATH, GRAPH_GET_CHANNEL_PATH,
	GRAPH_GET_NODE_PATH, GRAPH_LIST_CHANNELS_PATH, GRAPH_LIST_NODES_PATH, LIST_CHANNELS_PATH,
	LIST_FORWARDED_PAYMENTS_PATH, LIST_MACAROONS_PATH, LIST_PAYMENTS_PATH, LIST_PEERS_PATH,
	ONCHAIN_RECEIVE_PATH, ONCHAIN_SEND_PATH, OPEN_CHANNEL_PATH, REVOKE_MACAROON_PATH,
	SIGN_MESSAGE_PATH, SPLICE_IN_PATH, SPLICE_OUT_PATH, SPONTANEOUS_SEND_PATH,
	SUBSCRIBE_EVENTS_PATH, UNIFIED_SEND_PATH, UPDATE_CHANNEL_CONFIG_PATH, VERIFY_SIGNATURE_PATH,
};
use ldk_server_grpc::permissions::{
	CHANNELS_FORCE_CLOSE_PERMISSION, CHANNELS_MANAGE_PERMISSION, CHANNELS_READ_PERMISSION,
	CHANNELS_SPLICE_PERMISSION, EVENTS_READ_PERMISSION, GRAPH_READ_PERMISSION,
	INVOICES_CREATE_PERMISSION, MACAROONS_MANAGE_PERMISSION, MESSAGES_SIGN_PERMISSION,
	MESSAGES_VERIFY_PERMISSION, NODE_READ_PERMISSION, ONCHAIN_RECEIVE_PERMISSION,
	ONCHAIN_SEND_PERMISSION, PAYMENTS_CLAIM_PERMISSION, PAYMENTS_READ_PERMISSION,
	PAYMENTS_SEND_PERMISSION, PEERS_MANAGE_PERMISSION, PEERS_READ_PERMISSION,
	UTILITIES_READ_PERMISSION,
};

pub(crate) enum MethodAuthorization {
	Permission(&'static str),
	AuthenticatedOnly,
	Unknown,
}

pub(crate) fn method_authorization(method: &str) -> MethodAuthorization {
	match method {
		GET_NODE_INFO_PATH | GET_BALANCES_PATH | EXPORT_PATHFINDING_SCORES_PATH => {
			MethodAuthorization::Permission(NODE_READ_PERMISSION)
		},
		ONCHAIN_RECEIVE_PATH => MethodAuthorization::Permission(ONCHAIN_RECEIVE_PERMISSION),
		ONCHAIN_SEND_PATH => MethodAuthorization::Permission(ONCHAIN_SEND_PERMISSION),
		BOLT11_RECEIVE_PATH
		| BOLT11_RECEIVE_FOR_HASH_PATH
		| BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH
		| BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH
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
		| UNIFIED_SEND_PATH => MethodAuthorization::Permission(PAYMENTS_SEND_PERMISSION),
		GET_PAYMENT_DETAILS_PATH | LIST_PAYMENTS_PATH | LIST_FORWARDED_PAYMENTS_PATH => {
			MethodAuthorization::Permission(PAYMENTS_READ_PERMISSION)
		},
		LIST_CHANNELS_PATH => MethodAuthorization::Permission(CHANNELS_READ_PERMISSION),
		SPLICE_IN_PATH | SPLICE_OUT_PATH => {
			MethodAuthorization::Permission(CHANNELS_SPLICE_PERMISSION)
		},
		OPEN_CHANNEL_PATH | UPDATE_CHANNEL_CONFIG_PATH | CLOSE_CHANNEL_PATH => {
			MethodAuthorization::Permission(CHANNELS_MANAGE_PERMISSION)
		},
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

#[cfg(test)]
mod tests {
	use std::collections::BTreeSet;

	use ldk_server_grpc::permissions::ALL_PERMISSIONS;

	use super::*;
	use crate::macaroons::MacaroonInfo;
	#[test]
	fn every_rpc_has_the_expected_authorization() {
		// Keep this contract independent of the production mapping. The schema comparison
		// requires each new RPC to have an explicit authorization expectation here.
		let expected = [
			("GetNodeInfo", Some("node:read")),
			("GetBalances", Some("node:read")),
			("OnchainReceive", Some("onchain:receive")),
			("OnchainSend", Some("onchain:send")),
			("Bolt11Receive", Some("invoices:create")),
			("Bolt11ReceiveForHash", Some("invoices:create")),
			("Bolt11ClaimForId", Some("payments:claim")),
			("Bolt11FailForId", Some("payments:claim")),
			("Bolt11ReceiveViaJitChannel", Some("invoices:create")),
			("Bolt11ReceiveVariableAmountViaJitChannel", Some("invoices:create")),
			("Bolt11Send", Some("payments:send")),
			("Bolt11SendUnderpaying", Some("payments:send")),
			("Bolt12Receive", Some("invoices:create")),
			("Bolt12Send", Some("payments:send")),
			("Bolt12SendRefund", Some("payments:send")),
			("Bolt12ReceiveRefund", Some("invoices:create")),
			("Bolt12CreatePayerProof", Some("messages:sign")),
			("SpontaneousSend", Some("payments:send")),
			("OpenChannel", Some("channels:manage")),
			("SpliceIn", Some("channels:splice")),
			("SpliceOut", Some("channels:splice")),
			("UpdateChannelConfig", Some("channels:manage")),
			("CloseChannel", Some("channels:manage")),
			("ForceCloseChannel", Some("channels:force_close")),
			("ListChannels", Some("channels:read")),
			("GetPaymentDetails", Some("payments:read")),
			("ListPayments", Some("payments:read")),
			("ListForwardedPayments", Some("payments:read")),
			("ConnectPeer", Some("peers:manage")),
			("DisconnectPeer", Some("peers:manage")),
			("ListPeers", Some("peers:read")),
			("SignMessage", Some("messages:sign")),
			("VerifySignature", Some("messages:verify")),
			("ExportPathfindingScores", Some("node:read")),
			("UnifiedSend", Some("payments:send")),
			("DecodeInvoice", Some("utilities:read")),
			("DecodeOffer", Some("utilities:read")),
			("GraphListChannels", Some("graph:read")),
			("GraphGetChannel", Some("graph:read")),
			("GraphListNodes", Some("graph:read")),
			("GraphGetNode", Some("graph:read")),
			("SubscribeEvents", Some("events:read")),
			("CreateMacaroon", Some("macaroons:manage")),
			("ListMacaroons", Some("macaroons:manage")),
			("RevokeMacaroon", Some("macaroons:manage")),
			("GetPermissions", None),
		];
		let declared_methods: BTreeSet<_> =
			include_str!("../../../ldk-server-grpc/src/proto/api.proto")
				.lines()
				.filter_map(|line| {
					let mut words = line.split_whitespace();
					if words.next() != Some("rpc") {
						return None;
					}
					Some(words.next().expect("RPC name").split('(').next().unwrap())
				})
				.collect();
		let tested_methods: BTreeSet<_> = expected.iter().map(|(method, _)| *method).collect();
		assert_eq!(tested_methods.len(), expected.len(), "Duplicate RPC in permission table");
		assert_eq!(declared_methods, tested_methods, "Update the RPC permission test table");

		for (method, expected_permission) in expected {
			let required = match (method_authorization(method), expected_permission) {
				(MethodAuthorization::Permission(actual), Some(expected)) => {
					assert_eq!(actual, expected, "Incorrect permission for {method}");
					actual
				},
				(MethodAuthorization::AuthenticatedOnly, None) => continue,
				_ => panic!("Incorrect authorization classification for {method}"),
			};
			assert!(ALL_PERMISSIONS.contains(&required), "Unknown permission for {method}");
			let mut info = MacaroonInfo {
				id: "test".to_string(),
				name: "test".to_string(),
				permissions: BTreeSet::new(),
				caveats: Vec::new(),
			};
			assert!(
				!info.allows(required),
				"Identity without permissions must not access {method}"
			);
			for permission in ALL_PERMISSIONS {
				info.permissions = BTreeSet::from([permission.to_string()]);
				assert_eq!(
					info.allows(required),
					permission == "admin" || permission == required,
					"Unexpected access to {method} with {permission}"
				);
			}
		}
	}

	#[test]
	fn permissionless_and_unknown_methods_have_explicit_classification() {
		assert!(matches!(
			method_authorization(GET_PERMISSIONS_PATH),
			MethodAuthorization::AuthenticatedOnly
		));
		assert!(matches!(
			method_authorization("FutureUnclassifiedRpc"),
			MethodAuthorization::Unknown
		));
	}
}
