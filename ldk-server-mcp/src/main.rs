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
use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::mcp::{request_protocol_version, DiscoverResult, ListToolsResult, PROTOCOL_VERSION};
use crate::protocol::{
	JsonRpcErrorResponse, JsonRpcRequest, JsonRpcResponse, INVALID_PARAMS, METHOD_NOT_FOUND,
	PARSE_ERROR, UNSUPPORTED_PROTOCOL_VERSION,
};
use crate::tools::{build_tool_registry, ToolRegistry};

fn result_response(id: Value, result: impl Serialize) -> Value {
	serde_json::to_value(JsonRpcResponse::new(id, serde_json::to_value(result).unwrap())).unwrap()
}

fn error_response(id: Value, code: i64, message: impl Into<String>) -> Value {
	serde_json::to_value(JsonRpcErrorResponse::new(id, code, message.into())).unwrap()
}

fn error_response_with_data(
	id: Value, code: i64, message: impl Into<String>, data: Value,
) -> Value {
	serde_json::to_value(JsonRpcErrorResponse::with_data(id, code, message.into(), data)).unwrap()
}

async fn handle_request(
	request: JsonRpcRequest, client: &LdkServerClient, registry: &ToolRegistry,
) -> Value {
	let id = request.id.clone();
	if request.method == "initialize" {
		return error_response_with_data(
			id,
			METHOD_NOT_FOUND,
			format!("initialize is not supported; this server requires MCP {PROTOCOL_VERSION}"),
			serde_json::json!({ "supported": [PROTOCOL_VERSION] }),
		);
	}

	let protocol_version = match request_protocol_version(request.params.as_ref()) {
		Ok(protocol_version) => protocol_version,
		Err(message) => return error_response(id, INVALID_PARAMS, message),
	};
	if protocol_version != PROTOCOL_VERSION {
		return error_response_with_data(
			id,
			UNSUPPORTED_PROTOCOL_VERSION,
			"Unsupported protocol version",
			serde_json::json!({
				"supported": [PROTOCOL_VERSION],
				"requested": protocol_version,
			}),
		);
	}

	match request.method.as_str() {
		"server/discover" => result_response(id, DiscoverResult::new()),
		"tools/list" => result_response(id, ListToolsResult::new(registry.list_tools())),
		"tools/call" => {
			let params = request.params.as_ref().unwrap();
			let Some(tool_name) = params.get("name").and_then(Value::as_str) else {
				return error_response(id, INVALID_PARAMS, "Missing required parameter: name");
			};
			let tool_args = match params.get("arguments") {
				Some(arguments) if !arguments.is_object() => {
					return error_response(id, INVALID_PARAMS, "arguments must be an object");
				},
				Some(arguments) => arguments.clone(),
				None => serde_json::json!({}),
			};
			match registry.call_tool(client, tool_name, tool_args).await {
				Ok(result) => result_response(id, result),
				Err(e) => error_response(id, e.code, e.message),
			}
		},
		_ => error_response(id, METHOD_NOT_FOUND, format!("Method not found: {}", request.method)),
	}
}

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
	// still answers `server/discover` and `tools/list` even when the server is
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

		let response = match serde_json::from_str(trimmed) {
			Ok(message) => match JsonRpcRequest::from_value(message) {
				Ok(Some(request)) => handle_request(request, &client, &registry).await,
				Ok(None) => continue,
				Err(err) => serde_json::to_value(err).unwrap(),
			},
			Err(_) => error_response(Value::Null, PARSE_ERROR, "Parse error"),
		};

		let response = serde_json::to_string(&response).unwrap();
		let _ = stdout.write_all(response.as_bytes()).await;
		let _ = stdout.write_all(b"\n").await;
		let _ = stdout.flush().await;
	}
}
