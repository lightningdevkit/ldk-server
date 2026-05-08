// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use e2e_tests::{LdkServerHandle, McpHandle, TestBitcoind};
use ldk_server_client::ldk_server_grpc::api::Bolt11ReceiveRequest;
use ldk_server_client::ldk_server_grpc::types::{
	bolt11_invoice_description, Bolt11InvoiceDescription,
};
use serde_json::json;

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
	let node_info_text = node_info["result"]["content"][0]["text"].as_str().unwrap();
	let node_info_json: serde_json::Value = serde_json::from_str(node_info_text).unwrap();
	assert_eq!(node_info_json["node_id"], server.node_id());

	let onchain_receive = mcp.call(2, "tools/call", json!({
		"name": "onchain_receive",
		"arguments": {}
	}));
	let onchain_receive_text = onchain_receive["result"]["content"][0]["text"].as_str().unwrap();
	let onchain_receive_json: serde_json::Value =
		serde_json::from_str(onchain_receive_text).unwrap();
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
	let decode_invoice_text = decode_invoice["result"]["content"][0]["text"].as_str().unwrap();
	let decode_invoice_json: serde_json::Value =
		serde_json::from_str(decode_invoice_text).unwrap();
	assert_eq!(decode_invoice_json["destination"], server.node_id());
	assert_eq!(decode_invoice_json["description"], "mcp decode");
	assert_eq!(decode_invoice_json["amount_msat"], 50_000_000u64);
}
