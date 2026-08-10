// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

mod config;
mod mcp;
mod protocol;
mod tools;

use ldk_server_client::client::LdkServerClient;
use ldk_server_client::ldk_server_grpc::api::GetNodeInfoRequest;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::mcp::{
	InitializeResult, LEGACY_PROTOCOL_VERSION, PROTOCOL_VERSION, SERVER_NAME, SERVER_VERSION,
};
use crate::protocol::{
	JsonRpcErrorResponse, JsonRpcRequest, JsonRpcResponse, INVALID_PARAMS, METHOD_NOT_FOUND,
	PARSE_ERROR, UNSUPPORTED_PROTOCOL_VERSION,
};
use crate::tools::build_tool_registry;

#[tokio::main]
async fn main() {
	let mut config_path = None;
	let mut args = std::env::args().skip(1);
	while let Some(arg) = args.next() {
		match arg.as_str() {
			"--config" => {
				config_path = args.next();
				if config_path.is_none() {
					eprintln!("Error: --config requires a path argument");
					std::process::exit(1);
				}
			},
			other => {
				eprintln!("Unknown argument: {other}");
				std::process::exit(1);
			},
		}
	}

	let cfg = match config::resolve_config(config_path) {
		Ok(cfg) => cfg,
		Err(e) => {
			eprintln!("Error: {e}");
			std::process::exit(1);
		},
	};

	let client = match LdkServerClient::new(cfg.base_url, cfg.api_key, &cfg.tls_cert_pem) {
		Ok(c) => c,
		Err(e) => {
			eprintln!("Error: Failed to create client: {e}");
			std::process::exit(1);
		},
	};

	// Probe the server so misconfiguration surfaces on startup rather than on
	// the first tool call. We warn instead of exiting so the MCP protocol loop
	// still answers discovery and tool-list requests even when the server is
	// temporarily unreachable.
	if let Err(e) = client.get_node_info(GetNodeInfoRequest {}).await {
		eprintln!("Warning: Failed to reach ldk-server on startup: {e}");
	}

	let registry = build_tool_registry();

	eprintln!("ldk-server-mcp: ready, waiting for JSON-RPC requests on stdin");

	let stdin = tokio::io::stdin();
	let mut stdout = tokio::io::stdout();
	let mut reader = BufReader::new(stdin);
	let mut line = String::new();

	loop {
		line.clear();
		match reader.read_line(&mut line).await {
			Ok(0) => break, // EOF
			Ok(_) => {},
			Err(e) => {
				eprintln!("Error reading stdin: {e}");
				break;
			},
		}

		let trimmed = line.trim();
		if trimmed.is_empty() {
			continue;
		}

		let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
			Ok(r) => r,
			Err(_) => {
				let err =
					JsonRpcErrorResponse::new(Value::Null, PARSE_ERROR, "Parse error".to_string());
				let resp = serde_json::to_string(&err).unwrap();
				let _ = stdout.write_all(resp.as_bytes()).await;
				let _ = stdout.write_all(b"\n").await;
				let _ = stdout.flush().await;
				continue;
			},
		};

		// Notifications have no id — do not respond
		if request.id.is_none() {
			continue;
		}

		let id = request.id.unwrap();

		let protocol_version = request
			.params
			.as_ref()
			.and_then(|params| params.get("_meta"))
			.and_then(|meta| meta.get("io.modelcontextprotocol/protocolVersion"))
			.and_then(Value::as_str)
			.map(str::to_owned);

		if let Some(requested) = protocol_version.as_deref() {
			if requested != PROTOCOL_VERSION && requested != LEGACY_PROTOCOL_VERSION {
				let err = JsonRpcErrorResponse::with_data(
					id,
					UNSUPPORTED_PROTOCOL_VERSION,
					format!("Unsupported protocol version: {requested}"),
					serde_json::json!({
						"supported": [PROTOCOL_VERSION, LEGACY_PROTOCOL_VERSION],
						"requested": requested,
					}),
				);
				write_response(&mut stdout, serde_json::to_string(&err).unwrap()).await;
				continue;
			}

			let has_capabilities = request
				.params
				.as_ref()
				.and_then(|params| params.get("_meta"))
				.and_then(|meta| meta.get("io.modelcontextprotocol/clientCapabilities"))
				.is_some_and(Value::is_object);
			if requested == PROTOCOL_VERSION && !has_capabilities {
				let err = JsonRpcErrorResponse::new(
					id,
					INVALID_PARAMS,
					"Missing required request metadata: io.modelcontextprotocol/clientCapabilities"
						.to_string(),
				);
				write_response(&mut stdout, serde_json::to_string(&err).unwrap()).await;
				continue;
			}
		}

		let latest_protocol = protocol_version.as_deref() == Some(PROTOCOL_VERSION);
		let response_str = match request.method.as_str() {
			"initialize" => {
				if request
					.params
					.as_ref()
					.and_then(|params| params.get("protocolVersion"))
					.and_then(Value::as_str)
					== Some(PROTOCOL_VERSION)
				{
					let err = JsonRpcErrorResponse::new(
						id,
						METHOD_NOT_FOUND,
						"Method not found: initialize".to_string(),
					);
					serde_json::to_string(&err).unwrap()
				} else {
					let result = InitializeResult::new();
					let resp = JsonRpcResponse::new(id, serde_json::to_value(result).unwrap());
					serde_json::to_string(&resp).unwrap()
				}
			},
			"server/discover" => {
				if !latest_protocol {
					let err = JsonRpcErrorResponse::new(
						id,
						INVALID_PARAMS,
						"server/discover requires 2026-07-28 request metadata".to_string(),
					);
					serde_json::to_string(&err).unwrap()
				} else {
					let result = latest_result(serde_json::json!({
						"supportedVersions": [PROTOCOL_VERSION, LEGACY_PROTOCOL_VERSION],
						"capabilities": { "tools": {} },
						"instructions": "Use the available tools to operate an LDK Server node.",
						"ttlMs": 300_000,
						"cacheScope": "public",
					}));
					let resp = JsonRpcResponse::new(id, result);
					serde_json::to_string(&resp).unwrap()
				}
			},
			"tools/list" => {
				let tools = registry.list_tools();
				let result = if latest_protocol {
					latest_result(serde_json::json!({
						"tools": tools,
						"ttlMs": 300_000,
						"cacheScope": "public",
					}))
				} else {
					serde_json::json!({ "tools": tools })
				};
				let resp = JsonRpcResponse::new(id, result);
				serde_json::to_string(&resp).unwrap()
			},
			"ping" => {
				if latest_protocol {
					let err = JsonRpcErrorResponse::new(
						id,
						METHOD_NOT_FOUND,
						"Method not found: ping".to_string(),
					);
					serde_json::to_string(&err).unwrap()
				} else {
					let resp = JsonRpcResponse::new(id, serde_json::json!({}));
					serde_json::to_string(&resp).unwrap()
				}
			},
			"tools/call" => {
				let params = request.params.unwrap_or(Value::Null);
				match params.get("name").and_then(|v| v.as_str()) {
					Some(tool_name) if latest_protocol && !registry.has_tool(tool_name) => {
						let err = JsonRpcErrorResponse::new(
							id,
							INVALID_PARAMS,
							format!("Unknown tool: {tool_name}"),
						);
						serde_json::to_string(&err).unwrap()
					},
					Some(tool_name) => {
						let tool_args =
							params.get("arguments").cloned().unwrap_or(serde_json::json!({}));
						let result = registry.call_tool(&client, tool_name, tool_args).await;
						let mut result = serde_json::to_value(result).unwrap();
						if latest_protocol {
							result = latest_result(result);
						}
						let resp = JsonRpcResponse::new(id, result);
						serde_json::to_string(&resp).unwrap()
					},
					None => {
						let err = JsonRpcErrorResponse::new(
							id,
							INVALID_PARAMS,
							"Missing required parameter: name".to_string(),
						);
						serde_json::to_string(&err).unwrap()
					},
				}
			},
			_ => {
				let err = JsonRpcErrorResponse::new(
					id,
					METHOD_NOT_FOUND,
					format!("Method not found: {}", request.method),
				);
				serde_json::to_string(&err).unwrap()
			},
		};

		write_response(&mut stdout, response_str).await;
	}
}

fn latest_result(mut result: Value) -> Value {
	let object = result.as_object_mut().expect("MCP results must be JSON objects");
	object.insert("resultType".to_string(), Value::String("complete".to_string()));
	object.insert(
		"_meta".to_string(),
		serde_json::json!({
			"io.modelcontextprotocol/serverInfo": {
				"name": SERVER_NAME,
				"version": SERVER_VERSION,
			}
		}),
	);
	result
}

async fn write_response(stdout: &mut tokio::io::Stdout, response: String) {
	let _ = stdout.write_all(response.as_bytes()).await;
	let _ = stdout.write_all(b"\n").await;
	let _ = stdout.flush().await;
}
