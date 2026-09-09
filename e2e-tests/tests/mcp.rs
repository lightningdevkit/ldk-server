// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::str::FromStr;
use std::time::Duration;

use e2e_tests::{
	assert_replacement, mine_and_sync, payment_for_tx, setup_funded_channel, wait_for_event,
	wait_for_onchain_balance, wait_for_transaction, wait_for_wallet_sync, LdkServerHandle,
	McpHandle, TestBitcoind,
};
use ldk_node::lightning::offers::refund::Refund;
use ldk_server_client::ldk_server_grpc::api::{
	onchain_send_request, Bolt11ReceiveRequest, OnchainReceiveRequest, OnchainSendRequest,
};
use ldk_server_client::ldk_server_grpc::events::event_envelope::Event;
use ldk_server_client::ldk_server_grpc::types::{
	bolt11_invoice_description, payment_kind, Bolt11InvoiceDescription,
};
use serde_json::{json, Value};

fn tool_result_json(response: &Value) -> Value {
	let text = response["result"]["content"][0]["text"].as_str().unwrap();
	serde_json::from_str(text).unwrap()
}

#[tokio::test]
async fn test_mcp_initialize_and_list_tools() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;
	let mut mcp = McpHandle::start(&server);

	let initialize = mcp.call(
		1,
		"initialize",
		json!({
			"protocolVersion": "2025-11-25",
			"capabilities": {},
			"clientInfo": {"name": "e2e-test", "version": "0.1"}
		}),
	);
	assert_eq!(initialize["result"]["protocolVersion"], "2025-11-25");
	assert!(initialize["result"]["capabilities"]["tools"].is_object());

	let tools = mcp.call(2, "tools/list", json!({}));
	let tool_names = tools["result"]["tools"].as_array().unwrap();
	assert!(tool_names.iter().any(|tool| tool["name"] == "get_node_info"));
	assert!(tool_names.iter().any(|tool| tool["name"] == "onchain_receive"));
	assert!(tool_names.iter().any(|tool| tool["name"] == "decode_invoice"));
}

#[tokio::test]
async fn test_mcp_live_tool_calls() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;
	let mut mcp = McpHandle::start(&server);

	let node_info = mcp.call(1, "tools/call", json!({
		"name": "get_node_info",
		"arguments": {}
	}));
	let node_info_json = tool_result_json(&node_info);
	assert_eq!(node_info_json["node_id"], server.node_id());

	let onchain_receive = mcp.call(2, "tools/call", json!({
		"name": "onchain_receive",
		"arguments": {}
	}));
	let onchain_receive_json = tool_result_json(&onchain_receive);
	assert!(onchain_receive_json["address"].as_str().unwrap().starts_with("bcrt1"));

	let invoice = server
		.client()
		.bolt11_receive(Bolt11ReceiveRequest {
			amount_msat: Some(50_000_000),
			description: Some(Bolt11InvoiceDescription {
				kind: Some(bolt11_invoice_description::Kind::Direct("mcp decode".to_string())),
			}),
			expiry_secs: 3600,
		})
		.await
		.unwrap();

	let decode_invoice = mcp.call(3, "tools/call", json!({
		"name": "decode_invoice",
		"arguments": { "invoice": invoice.invoice }
	}));
	let decode_invoice_json = tool_result_json(&decode_invoice);
	assert_eq!(decode_invoice_json["destination"], server.node_id());
	assert_eq!(decode_invoice_json["description"], "mcp decode");
	assert_eq!(decode_invoice_json["amount_msat"], 50_000_000u64);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_mcp_bolt12_refund() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	let mut events_a = server_a.client().subscribe_events().await.unwrap();
	let mut events_b = server_b.client().subscribe_events().await.unwrap();

	setup_funded_channel(&bitcoind, &server_b, &server_a, 100_000).await;

	let mut mcp_a = McpHandle::start(&server_a);
	let mut mcp_b = McpHandle::start(&server_b);
	let send_refund = mcp_b.call(
		1,
		"tools/call",
		json!({
			"name": "bolt12_send_refund",
			"arguments": {
				"amount_msat": 5_000_000,
				"quantity": 1,
				"payer_note": "mcp refund"
			}
		}),
	);
	let send_refund = tool_result_json(&send_refund);
	let refund_str = send_refund["refund"].as_str().unwrap();
	let refund = Refund::from_str(refund_str).unwrap();
	assert_eq!(refund.amount_msats(), 5_000_000);
	assert_eq!(refund.quantity(), Some(1));
	assert_eq!(refund.payer_note().unwrap().to_string(), "mcp refund");

	let receive_refund = mcp_a.call(
		1,
		"tools/call",
		json!({
			"name": "bolt12_receive_refund",
			"arguments": { "refund": refund_str }
		}),
	);
	let receive_refund = tool_result_json(&receive_refund);
	let payment_hash = receive_refund["payment_hash"].as_str().unwrap();

	let event_a =
		wait_for_event(&mut events_a, |event| matches!(event, Event::PaymentReceived(_))).await;
	let Some(Event::PaymentReceived(payment_received)) = event_a.event else {
		panic!("expected PaymentReceived");
	};
	let payment = payment_received.payment.unwrap();
	let Some(payment_kind::Kind::Bolt12Refund(refund)) = payment.kind.unwrap().kind else {
		panic!("expected BOLT12 refund kind");
	};
	assert_eq!(refund.hash.as_deref(), Some(payment_hash));
	wait_for_event(&mut events_b, |event| matches!(event, Event::PaymentSuccessful(_))).await;
}

#[tokio::test]
async fn test_mcp_onchain_fee_bump() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;
	let address = server.client().onchain_receive(OnchainReceiveRequest {}).await.unwrap().address;
	bitcoind.fund_address(&address, 1.0);
	mine_and_sync(&bitcoind, &[&server], 6).await;
	wait_for_onchain_balance(server.client(), Duration::from_secs(30)).await;
	let destination = bitcoind.bitcoind.client.new_address().unwrap().to_string();
	let original = server
		.client()
		.onchain_send(OnchainSendRequest {
			address: destination.clone(),
			amount: Some(onchain_send_request::Amount::AmountSats(50_000)),
			fee_rate_sat_per_vb: Some(2),
		})
		.await
		.unwrap()
		.txid;
	wait_for_transaction(&bitcoind, &original).await;
	let payment = payment_for_tx(&server, &original).await;
	let mut mcp = McpHandle::start(&server);
	let mut previous = original;
	for rate in [Some(5), None] {
		let mut arguments = json!({"payment_id": payment.payment_id});
		if let Some(rate) = rate {
			arguments["fee_rate_sat_per_vb"] = json!(rate);
		}
		let response = mcp.call(
			1,
			"tools/call",
			json!({
				"name": "onchain_bump_fee", "arguments": arguments
			}),
		);
		assert_ne!(response["result"]["isError"], true, "{response}");
		let output = tool_result_json(&response);
		let replacement = output["txid"].as_str().unwrap();
		assert_replacement(&bitcoind, &previous, replacement, &destination).await;
		let updated = payment_for_tx(&server, replacement).await;
		assert_eq!(updated.payment_id, payment.payment_id);
		assert_eq!(updated.amount_msat, Some(50_000_000));
		assert!(updated.fee_paid_msat > payment.fee_paid_msat);
		previous = replacement.to_string();
		if rate.is_some() {
			wait_for_wallet_sync(&server).await;
		}
	}
}
