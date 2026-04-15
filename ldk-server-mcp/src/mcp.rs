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

pub const PROTOCOL_VERSION: &str = "2025-11-25";
pub const SERVER_NAME: &str = "ldk-server-mcp";
pub const SERVER_VERSION: &str = "0.1.0";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
	pub protocol_version: String,
	pub capabilities: Capabilities,
	pub server_info: ServerInfo,
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

impl InitializeResult {
	pub fn new() -> Self {
		Self {
			protocol_version: PROTOCOL_VERSION.to_string(),
			capabilities: Capabilities { tools: ToolsCapability {} },
			server_info: ServerInfo {
				name: SERVER_NAME.to_string(),
				version: SERVER_VERSION.to_string(),
			},
		}
	}
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
pub struct ToolCallResult {
	pub content: Vec<ToolContent>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub is_error: Option<bool>,
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
			content: vec![ToolContent { content_type: "text".to_string(), text }],
			is_error: None,
		}
	}

	pub fn error(text: String) -> Self {
		Self {
			content: vec![ToolContent { content_type: "text".to_string(), text }],
			is_error: Some(true),
		}
	}
}
