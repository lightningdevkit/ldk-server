// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use ldk_server_client::error::{LdkServerError, LdkServerErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PARSE_ERROR: i64 = -32700;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

/// Classified error produced by MCP tool handlers. The `code` is reused for JSON-RPC error
/// responses at the envelope level, and for categorising the error text that gets surfaced
/// through a `ToolCallResult` with `isError: true`.
#[derive(Debug)]
pub struct McpError {
	pub code: i64,
	pub message: String,
}

impl McpError {
	pub fn invalid_params(message: impl Into<String>) -> Self {
		Self { code: INVALID_PARAMS, message: message.into() }
	}

	pub fn internal(message: impl Into<String>) -> Self {
		Self { code: INTERNAL_ERROR, message: message.into() }
	}

	pub fn category(&self) -> &'static str {
		match self.code {
			INVALID_PARAMS => "Invalid params",
			INTERNAL_ERROR => "Internal error",
			_ => "Error",
		}
	}
}

impl From<LdkServerError> for McpError {
	fn from(e: LdkServerError) -> Self {
		let code = match e.error_code {
			LdkServerErrorCode::InvalidRequestError => INVALID_PARAMS,
			LdkServerErrorCode::AuthError
			| LdkServerErrorCode::LightningError
			| LdkServerErrorCode::InternalServerError
			| LdkServerErrorCode::InternalError => INTERNAL_ERROR,
		};
		Self { code, message: e.message }
	}
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
	#[allow(dead_code)]
	pub jsonrpc: String,
	pub id: Option<Value>,
	pub method: String,
	pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
	pub jsonrpc: String,
	pub id: Value,
	pub result: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcErrorResponse {
	pub jsonrpc: String,
	pub id: Value,
	pub error: JsonRpcError,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
	pub code: i64,
	pub message: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub data: Option<Value>,
}

impl JsonRpcResponse {
	pub fn new(id: Value, result: Value) -> Self {
		Self { jsonrpc: "2.0".to_string(), id, result }
	}
}

impl JsonRpcErrorResponse {
	pub fn new(id: Value, code: i64, message: String) -> Self {
		Self { jsonrpc: "2.0".to_string(), id, error: JsonRpcError { code, message, data: None } }
	}
}
