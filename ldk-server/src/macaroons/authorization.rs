// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! RPC permission requirements.

pub(crate) use ldk_server_grpc::permissions::{method_authorization, MethodAuthorization};

#[cfg(test)]
mod tests {
	use std::collections::BTreeSet;

	use ldk_server_grpc::endpoints::GET_PERMISSIONS_PATH;
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
			("OnchainBumpFee", Some("onchain:send")),
			("Bolt11Receive", Some("invoices:create")),
			("Bolt11ReceiveForHash", Some("invoices:create")),
			("Bolt11ClaimForId", Some("payments:claim")),
			("Bolt11FailForId", Some("payments:claim")),
			("Bolt11ReceiveViaJitChannel", Some("invoices:create")),
			("Bolt11ReceiveVariableAmountViaJitChannel", Some("invoices:create")),
			("Bolt11ReceiveViaJitChannelForHash", Some("invoices:create")),
			("Bolt11ReceiveVariableAmountViaJitChannelForHash", Some("invoices:create")),
			("Bolt11Send", Some("payments:send")),
			("Bolt11SendUnderpaying", Some("payments:send")),
			("Bolt12Receive", Some("invoices:create")),
			("Bolt12Send", Some("payments:send")),
			("Bolt12SendRefund", Some("payments:send")),
			("Bolt12ReceiveRefund", Some("invoices:create")),
			("Bolt12CreatePayerProof", Some("messages:sign")),
			("SpontaneousSend", Some("payments:send")),
			("OpenChannel", Some("channels:manage")),
			("SpliceIn", Some("channels:manage")),
			("SpliceOut", Some("payments:send")),
			("BumpChannelFundingFee", Some("channels:manage")),
			("UpdateChannelConfig", Some("channels:manage")),
			("CloseChannel", Some("channels:manage")),
			("ForceCloseChannel", Some("channels:force_close")),
			("ListChannels", Some("channels:read")),
			("GetPaymentDetails", Some("payments:read")),
			("ListPayments", Some("payments:read")),
			("ListForwardedPayments", Some("payments:read")),
			("GetForwardedPaymentDetails", Some("payments:read")),
			("GetForwardedPaymentTrackingMode", Some("payments:read")),
			("GetChannelForwardingStats", Some("payments:read")),
			("ListChannelForwardingStats", Some("payments:read")),
			("ListChannelPairForwardingStats", Some("payments:read")),
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
