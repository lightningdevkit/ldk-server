// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use serde::Serialize;
use serde_json::Value;

pub const PROTOCOL_VERSION: &str = "2026-07-28";
pub const SERVER_NAME: &str = "ldk-server-mcp";
pub const SERVER_VERSION: &str = "0.1.0";
pub const CACHE_TTL_MS: u64 = 3_600_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverResult {
	pub result_type: &'static str,
	pub supported_versions: [&'static str; 1],
	pub capabilities: Capabilities,
	#[serde(rename = "_meta")]
	pub metadata: ResultMetadata,
	pub ttl_ms: u64,
	pub cache_scope: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Capabilities {
	pub tools: ToolsCapability,
}

#[derive(Debug, Serialize)]
pub struct ToolsCapability {}

#[derive(Debug, Serialize)]
pub struct ServerInfo {
	pub name: String,
	pub version: String,
}

#[derive(Debug, Serialize)]
pub struct ResultMetadata {
	#[serde(rename = "io.modelcontextprotocol/serverInfo")]
	pub server_info: ServerInfo,
}

impl ServerInfo {
	pub fn new() -> Self {
		Self { name: SERVER_NAME.to_string(), version: SERVER_VERSION.to_string() }
	}
}

impl ResultMetadata {
	pub fn new() -> Self {
		Self { server_info: ServerInfo::new() }
	}
}

impl DiscoverResult {
	pub fn new() -> Self {
		Self {
			result_type: "complete",
			supported_versions: [PROTOCOL_VERSION],
			capabilities: Capabilities { tools: ToolsCapability {} },
			metadata: ResultMetadata::new(),
			ttl_ms: CACHE_TTL_MS,
			cache_scope: "public",
		}
	}
}

pub fn request_protocol_version(params: Option<&Value>) -> Result<&str, String> {
	let params = params.and_then(Value::as_object).ok_or("params must be an object")?;
	let metadata = params
		.get("_meta")
		.and_then(Value::as_object)
		.ok_or("Missing or invalid required parameter: _meta")?;

	let protocol_version = metadata
		.get("io.modelcontextprotocol/protocolVersion")
		.and_then(Value::as_str)
		.ok_or("Missing or invalid _meta.io.modelcontextprotocol/protocolVersion")?;

	if !metadata.get("io.modelcontextprotocol/clientCapabilities").is_some_and(Value::is_object) {
		return Err(
			"Missing or invalid _meta.io.modelcontextprotocol/clientCapabilities".to_string()
		);
	}

	if let Some(client_info) = metadata.get("io.modelcontextprotocol/clientInfo") {
		let valid = client_info.as_object().is_some_and(|client_info| {
			client_info.get("name").is_some_and(Value::is_string)
				&& client_info.get("version").is_some_and(Value::is_string)
		});
		if !valid {
			return Err("Invalid _meta.io.modelcontextprotocol/clientInfo".to_string());
		}
	}

	Ok(protocol_version)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
	pub name: String,
	pub description: String,
	pub input_schema: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult<'a> {
	pub result_type: &'static str,
	pub tools: &'a [ToolDefinition],
	pub ttl_ms: u64,
	pub cache_scope: &'static str,
	#[serde(rename = "_meta")]
	pub metadata: ResultMetadata,
}

impl<'a> ListToolsResult<'a> {
	pub fn new(tools: &'a [ToolDefinition]) -> Self {
		Self {
			result_type: "complete",
			tools,
			ttl_ms: CACHE_TTL_MS,
			cache_scope: "public",
			metadata: ResultMetadata::new(),
		}
	}
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
	pub result_type: &'static str,
	pub content: Vec<ToolContent>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub is_error: Option<bool>,
	#[serde(rename = "_meta")]
	pub metadata: ResultMetadata,
}

#[derive(Debug, Serialize)]
pub struct ToolContent {
	#[serde(rename = "type")]
	pub content_type: String,
	pub text: String,
}

impl ToolCallResult {
	pub fn success(text: String) -> Self {
		Self {
			result_type: "complete",
			content: vec![ToolContent { content_type: "text".to_string(), text }],
			is_error: None,
			metadata: ResultMetadata::new(),
		}
	}

	pub fn execution_error(text: String) -> Self {
		Self {
			result_type: "complete",
			content: vec![ToolContent { content_type: "text".to_string(), text }],
			is_error: Some(true),
			metadata: ResultMetadata::new(),
		}
	}
}
