// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::io::{BufRead, BufReader, Write};

use serde_json::{json, Value};

const PROTOCOL_VERSION: &str = "2026-07-28";
const NUM_TOOLS: usize = 37;
const EXPECTED_TOOLS: [&str; NUM_TOOLS] = [
	"bolt11_claim_for_hash",
	"bolt11_fail_for_hash",
	"bolt11_receive",
	"bolt11_receive_for_hash",
	"bolt11_receive_variable_amount_via_jit_channel",
	"bolt11_receive_via_jit_channel",
	"bolt11_send",
	"bolt12_receive",
	"bolt12_send",
	"close_channel",
	"connect_peer",
	"decode_invoice",
	"decode_offer",
	"disconnect_peer",
	"export_pathfinding_scores",
	"force_close_channel",
	"get_balances",
	"get_node_info",
	"get_payment_details",
	"graph_get_channel",
	"graph_get_node",
	"graph_list_channels",
	"graph_list_nodes",
	"list_channels",
	"list_forwarded_payments",
	"list_payments",
	"list_peers",
	"onchain_receive",
	"onchain_send",
	"open_channel",
	"sign_message",
	"splice_in",
	"splice_out",
	"spontaneous_send",
	"unified_send",
	"update_channel_config",
	"verify_signature",
];

fn test_cert_path() -> String {
	std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("tests/fixtures/test_cert.pem")
		.to_str()
		.unwrap()
		.to_string()
}

struct McpProcess {
	child: std::process::Child,
	stdin: std::process::ChildStdin,
	reader: BufReader<std::process::ChildStdout>,
}

impl McpProcess {
	fn spawn() -> Self {
		let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_ldk-server-mcp"))
			.env("LDK_BASE_URL", "localhost:19999")
			.env("LDK_API_KEY", "deadbeef")
			.env("LDK_TLS_CERT_PATH", test_cert_path())
			.stdin(std::process::Stdio::piped())
			.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::piped())
			.spawn()
			.expect("Failed to spawn MCP process");

		let stdin = child.stdin.take().unwrap();
		let stdout = child.stdout.take().unwrap();
		let reader = BufReader::new(stdout);

		McpProcess { child, stdin, reader }
	}

	fn send(&mut self, msg: &Value) {
		let mut msg = msg.clone();
		if msg.get("id").is_some() {
			let params = msg.as_object_mut().unwrap().entry("params").or_insert_with(|| json!({}));
			params.as_object_mut().unwrap().entry("_meta").or_insert_with(|| {
				json!({
					"io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
					"io.modelcontextprotocol/clientInfo": {
						"name": "ldk-server-mcp-test",
						"version": "0.1.0"
					},
					"io.modelcontextprotocol/clientCapabilities": {}
				})
			});
		}
		self.send_raw(&msg);
	}

	fn send_raw(&mut self, msg: &Value) {
		let line = serde_json::to_string(msg).unwrap();
		writeln!(self.stdin, "{}", line).expect("Failed to write to stdin");
		self.stdin.flush().expect("Failed to flush stdin");
	}

	fn recv(&mut self) -> Value {
		let mut line = String::new();
		self.reader.read_line(&mut line).expect("Failed to read from stdout");
		serde_json::from_str(line.trim()).expect("Failed to parse JSON response")
	}
}

impl Drop for McpProcess {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}

fn assert_unreachable_tool(tool_name: &str, arguments: Value) {
	let mut proc = McpProcess::spawn();

	proc.send(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/call",
		"params": {
			"name": tool_name,
			"arguments": arguments
		}
	}));

	let resp = proc.recv();
	assert_eq!(resp["jsonrpc"], "2.0");
	assert_eq!(resp["id"], 1);
	assert_eq!(resp["result"]["isError"], true);
	let text = resp["result"]["content"][0]["text"].as_str().unwrap();
	assert!(!text.is_empty(), "Expected non-empty error message");
}

#[test]
fn test_server_discover() {
	let mut proc = McpProcess::spawn();

	proc.send(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "server/discover",
		"params": {}
	}));

	let resp = proc.recv();
	assert_eq!(resp["jsonrpc"], "2.0");
	assert_eq!(resp["id"], 1);
	assert_eq!(resp["result"]["resultType"], "complete");
	assert_eq!(resp["result"]["supportedVersions"], json!([PROTOCOL_VERSION]));
	assert!(resp["result"]["capabilities"]["tools"].is_object());
	assert_eq!(
		resp["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
		"ldk-server-mcp"
	);
	assert_eq!(resp["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["version"], "0.1.0");
	assert_eq!(resp["result"]["ttlMs"], 3_600_000);
	assert_eq!(resp["result"]["cacheScope"], "public");
}

#[test]
fn test_tools_list() {
	let mut proc = McpProcess::spawn();

	proc.send(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/list",
		"params": {}
	}));

	let resp = proc.recv();
	assert_eq!(resp["jsonrpc"], "2.0");
	assert_eq!(resp["id"], 1);
	assert_eq!(resp["result"]["resultType"], "complete");
	assert_eq!(resp["result"]["ttlMs"], 3_600_000);
	assert_eq!(resp["result"]["cacheScope"], "public");
	assert_eq!(
		resp["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
		"ldk-server-mcp"
	);

	let tools = resp["result"]["tools"].as_array().unwrap();
	assert_eq!(tools.len(), NUM_TOOLS, "Expected {NUM_TOOLS} tools, got {}", tools.len());
	let mut tool_names = tools
		.iter()
		.map(|tool| tool["name"].as_str().expect("Tool missing name").to_string())
		.collect::<Vec<_>>();
	tool_names.sort();

	let mut expected_tool_names =
		EXPECTED_TOOLS.iter().map(|name| name.to_string()).collect::<Vec<_>>();
	expected_tool_names.sort();
	assert_eq!(tool_names, expected_tool_names, "Tool names drifted from the expected API surface");

	for tool in tools {
		assert!(tool["name"].is_string(), "Tool missing name");
		assert!(tool["description"].is_string(), "Tool missing description");
		assert!(tool["inputSchema"].is_object(), "Tool missing inputSchema");
	}
}

#[test]
fn test_removed_ping_returns_method_not_found() {
	let mut proc = McpProcess::spawn();

	proc.send(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "ping"
	}));

	let resp = proc.recv();
	assert_eq!(resp["jsonrpc"], "2.0");
	assert_eq!(resp["id"], 1);
	assert_eq!(resp["error"]["code"], -32601);
	assert!(resp["error"]["message"].as_str().unwrap().contains("ping"));
}

#[test]
fn test_initialize_reports_supported_modern_version() {
	let mut proc = McpProcess::spawn();

	proc.send_raw(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "initialize",
		"params": {
			"protocolVersion": "2025-11-25",
			"capabilities": {},
			"clientInfo": {"name": "legacy-test", "version": "0.1.0"}
		}
	}));

	let resp = proc.recv();
	assert_eq!(resp["error"]["code"], -32601);
	assert_eq!(resp["error"]["data"]["supported"], json!([PROTOCOL_VERSION]));
}

#[test]
fn test_missing_request_metadata_is_invalid_params() {
	let mut proc = McpProcess::spawn();

	proc.send_raw(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/list",
		"params": {}
	}));

	let resp = proc.recv();
	assert_eq!(resp["error"]["code"], -32602);
	assert!(resp["error"]["message"].as_str().unwrap().contains("_meta"));
}

#[test]
fn test_unsupported_protocol_version_reports_supported_versions() {
	let mut proc = McpProcess::spawn();

	proc.send_raw(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/list",
		"params": {
			"_meta": {
				"io.modelcontextprotocol/protocolVersion": "1900-01-01",
				"io.modelcontextprotocol/clientCapabilities": {}
			}
		}
	}));

	let resp = proc.recv();
	assert_eq!(resp["error"]["code"], -32022);
	assert_eq!(resp["error"]["data"]["supported"], json!([PROTOCOL_VERSION]));
	assert_eq!(resp["error"]["data"]["requested"], "1900-01-01");
}

#[test]
fn test_missing_client_capabilities_is_invalid_params() {
	let mut proc = McpProcess::spawn();

	proc.send_raw(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/list",
		"params": {
			"_meta": {
				"io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION
			}
		}
	}));

	let resp = proc.recv();
	assert_eq!(resp["error"]["code"], -32602);
	assert!(resp["error"]["message"].as_str().unwrap().contains("clientCapabilities"));
}

#[test]
fn test_malformed_client_info_is_invalid_params() {
	let mut proc = McpProcess::spawn();

	proc.send_raw(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/list",
		"params": {
			"_meta": {
				"io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
				"io.modelcontextprotocol/clientInfo": {"name": "missing-version"},
				"io.modelcontextprotocol/clientCapabilities": {}
			}
		}
	}));

	let resp = proc.recv();
	assert_eq!(resp["error"]["code"], -32602);
	assert!(resp["error"]["message"].as_str().unwrap().contains("clientInfo"));
}

#[test]
fn test_invalid_json_rpc_envelope_is_invalid_request() {
	let mut proc = McpProcess::spawn();

	proc.send_raw(&json!({
		"jsonrpc": "1.0",
		"id": 1,
		"method": "tools/list",
		"params": {
			"_meta": {
				"io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
				"io.modelcontextprotocol/clientCapabilities": {}
			}
		}
	}));

	let resp = proc.recv();
	assert_eq!(resp["error"]["code"], -32600);
}

#[test]
fn test_non_object_params_are_invalid_request() {
	let mut proc = McpProcess::spawn();

	proc.send_raw(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/list",
		"params": []
	}));

	let resp = proc.recv();
	assert_eq!(resp["error"]["code"], -32600);
}

#[test]
fn test_tools_call_unknown_tool() {
	let mut proc = McpProcess::spawn();

	proc.send(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/call",
		"params": {
			"name": "nonexistent_tool",
			"arguments": {}
		}
	}));

	let resp = proc.recv();
	assert_eq!(resp["jsonrpc"], "2.0");
	assert_eq!(resp["id"], 1);
	assert_eq!(resp["error"]["code"], -32602);
	assert!(resp["error"]["message"].as_str().unwrap().contains("Unknown tool"));
}

#[test]
fn test_tools_call_unreachable_server() {
	let mut proc = McpProcess::spawn();

	proc.send(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/call",
		"params": {
			"name": "get_node_info",
			"arguments": {}
		}
	}));

	let resp = proc.recv();
	assert_eq!(resp["jsonrpc"], "2.0");
	assert_eq!(resp["id"], 1);
	assert_eq!(resp["result"]["resultType"], "complete");
	assert_eq!(resp["result"]["isError"], true);
	assert_eq!(
		resp["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
		"ldk-server-mcp"
	);
	let text = resp["result"]["content"][0]["text"].as_str().unwrap();
	assert!(!text.is_empty(), "Expected non-empty error message");
}

#[test]
fn test_bolt11_receive_via_jit_channel_unreachable() {
	let mut proc = McpProcess::spawn();

	proc.send(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/call",
		"params": {
			"name": "bolt11_receive_via_jit_channel",
			"arguments": {
				"amount_msat": 1000,
				"description": {"kind": {"direct": "test jit"}}
			}
		}
	}));

	let resp = proc.recv();
	assert_eq!(resp["jsonrpc"], "2.0");
	assert_eq!(resp["id"], 1);
	assert_eq!(resp["result"]["isError"], true);
	let text = resp["result"]["content"][0]["text"].as_str().unwrap();
	assert!(!text.is_empty(), "Expected non-empty error message");
}

#[test]
fn test_bolt11_receive_variable_amount_via_jit_channel_unreachable() {
	assert_unreachable_tool(
		"bolt11_receive_variable_amount_via_jit_channel",
		json!({ "description": {"kind": {"direct": "test jit"}} }),
	);
}

#[test]
fn test_bolt11_receive_for_hash_unreachable() {
	assert_unreachable_tool(
		"bolt11_receive_for_hash",
		json!({
			"payment_hash": "00".repeat(32),
			"description": {"kind": {"direct": "test hodl"}}
		}),
	);
}

#[test]
fn test_bolt11_claim_for_hash_unreachable() {
	assert_unreachable_tool(
		"bolt11_claim_for_hash",
		json!({
			"payment_hash": "11".repeat(32),
			"preimage": "22".repeat(32)
		}),
	);
}

#[test]
fn test_bolt11_fail_for_hash_unreachable() {
	assert_unreachable_tool("bolt11_fail_for_hash", json!({ "payment_hash": "33".repeat(32) }));
}

#[test]
fn test_unified_send_unreachable() {
	assert_unreachable_tool("unified_send", json!({ "uri": "bitcoin:tb1qexample?amount=0.001" }));
}

#[test]
fn test_list_peers_unreachable() {
	assert_unreachable_tool("list_peers", json!({}));
}

#[test]
fn test_decode_invoice_unreachable() {
	assert_unreachable_tool("decode_invoice", json!({ "invoice": "lnbc1example" }));
}

#[test]
fn test_decode_offer_unreachable() {
	assert_unreachable_tool("decode_offer", json!({ "offer": "lno1example" }));
}

#[test]
fn test_notification_no_response() {
	let mut proc = McpProcess::spawn();

	// Send a notification (no id) - should produce no response.
	proc.send(&json!({
		"jsonrpc": "2.0",
		"method": "notifications/cancelled",
		"params": {"requestId": 999}
	}));

	// Send a real request after the notification
	proc.send(&json!({
		"jsonrpc": "2.0",
		"id": 42,
		"method": "tools/list",
		"params": {}
	}));

	// The first response we get should be for id 42, not for the notification
	let resp = proc.recv();
	assert_eq!(resp["id"], 42);
}

#[test]
fn test_json_rpc_batch_is_invalid_request() {
	let mut proc = McpProcess::spawn();

	proc.send_raw(&json!([{
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/list",
		"params": {}
	}]));

	let resp = proc.recv();
	assert_eq!(resp["error"]["code"], -32600);
}

#[test]
fn test_graph_list_channels_unreachable() {
	let mut proc = McpProcess::spawn();

	proc.send(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/call",
		"params": {
			"name": "graph_list_channels",
			"arguments": {}
		}
	}));

	let resp = proc.recv();
	assert_eq!(resp["jsonrpc"], "2.0");
	assert_eq!(resp["id"], 1);
	assert_eq!(resp["result"]["isError"], true);
	let text = resp["result"]["content"][0]["text"].as_str().unwrap();
	assert!(!text.is_empty(), "Expected non-empty error message");
}

#[test]
fn test_graph_get_channel_unreachable() {
	let mut proc = McpProcess::spawn();

	proc.send(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/call",
		"params": {
			"name": "graph_get_channel",
			"arguments": {"short_channel_id": 12345}
		}
	}));

	let resp = proc.recv();
	assert_eq!(resp["jsonrpc"], "2.0");
	assert_eq!(resp["id"], 1);
	assert_eq!(resp["result"]["isError"], true);
	let text = resp["result"]["content"][0]["text"].as_str().unwrap();
	assert!(!text.is_empty(), "Expected non-empty error message");
}

#[test]
fn test_graph_list_nodes_unreachable() {
	let mut proc = McpProcess::spawn();

	proc.send(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/call",
		"params": {
			"name": "graph_list_nodes",
			"arguments": {}
		}
	}));

	let resp = proc.recv();
	assert_eq!(resp["jsonrpc"], "2.0");
	assert_eq!(resp["id"], 1);
	assert_eq!(resp["result"]["isError"], true);
	let text = resp["result"]["content"][0]["text"].as_str().unwrap();
	assert!(!text.is_empty(), "Expected non-empty error message");
}

#[test]
fn test_graph_get_node_unreachable() {
	let mut proc = McpProcess::spawn();

	proc.send(&json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "tools/call",
		"params": {
			"name": "graph_get_node",
			"arguments": {"node_id": "02deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"}
		}
	}));

	let resp = proc.recv();
	assert_eq!(resp["jsonrpc"], "2.0");
	assert_eq!(resp["id"], 1);
	assert_eq!(resp["result"]["isError"], true);
	let text = resp["result"]["content"][0]["text"].as_str().unwrap();
	assert!(!text.is_empty(), "Expected non-empty error message");
}

#[test]
fn test_malformed_json() {
	let mut proc = McpProcess::spawn();

	// Send garbage
	writeln!(proc.stdin, "this is not json").unwrap();
	proc.stdin.flush().unwrap();

	let resp = proc.recv();
	assert_eq!(resp["jsonrpc"], "2.0");
	assert!(resp["error"].is_object());
	assert_eq!(resp["error"]["code"], -32700);
	assert_eq!(resp["error"]["message"], "Parse error");
}
