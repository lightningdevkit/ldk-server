// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

pub mod handlers;
pub mod schema;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use ldk_server_client::client::LdkServerClient;
use serde_json::Value;

use crate::mcp::{ToolCallResult, ToolDefinition};
use crate::protocol::McpError;

type ToolHandler = for<'a> fn(
	&'a LdkServerClient,
	Value,
)
	-> Pin<Box<dyn Future<Output = Result<Value, McpError>> + Send + 'a>>;

pub struct ToolRegistry {
	definitions: Vec<ToolDefinition>,
	handlers: HashMap<&'static str, ToolHandler>,
}

struct ToolSpec {
	name: &'static str,
	description: &'static str,
	input_schema: fn() -> Value,
	handler: ToolHandler,
}

fn tool_spec(
	name: &'static str, description: &'static str, input_schema: fn() -> Value,
	handler: ToolHandler,
) -> ToolSpec {
	ToolSpec { name, description, input_schema, handler }
}

impl ToolRegistry {
	pub fn list_tools(&self) -> &[ToolDefinition] {
		&self.definitions
	}

	pub async fn call_tool(
		&self, client: &LdkServerClient, name: &str, args: Value,
	) -> ToolCallResult {
		let Some(handler) = self.handlers.get(name) else {
			return ToolCallResult::error(format!("Unknown tool: {name}"));
		};
		match handler(client, args).await {
			Ok(value) => {
				let text = serde_json::to_string(&value)
					.unwrap_or_else(|e| format!("Failed to serialize response: {e}"));
				ToolCallResult::success(text)
			},
			Err(e) => ToolCallResult::error(format!("{}: {}", e.category(), e.message)),
		}
	}
}

pub fn build_tool_registry() -> ToolRegistry {
	let tools = vec![
		tool_spec(
			"get_node_info",
			"Retrieve node info including node_id, sync status, and best block",
			schema::get_node_info_schema,
			|client, args| Box::pin(handlers::handle_get_node_info(client, args)),
		),
		tool_spec(
			"get_balances",
			"Retrieve an overview of all known balances (on-chain and Lightning)",
			schema::get_balances_schema,
			|client, args| Box::pin(handlers::handle_get_balances(client, args)),
		),
		tool_spec(
			"onchain_receive",
			"Generate a new on-chain Bitcoin funding address",
			schema::onchain_receive_schema,
			|client, args| Box::pin(handlers::handle_onchain_receive(client, args)),
		),
		tool_spec(
			"onchain_send",
			"Send an on-chain Bitcoin payment to an address",
			schema::onchain_send_schema,
			|client, args| Box::pin(handlers::handle_onchain_send(client, args)),
		),
		tool_spec(
			"bolt11_receive",
			"Create a BOLT11 Lightning invoice to receive a payment",
			schema::bolt11_receive_schema,
			|client, args| Box::pin(handlers::handle_bolt11_receive(client, args)),
		),
		tool_spec(
			"bolt11_receive_for_hash",
			"Create a BOLT11 Lightning invoice for a specific payment hash",
			schema::bolt11_receive_for_hash_schema,
			|client, args| Box::pin(handlers::handle_bolt11_receive_for_hash(client, args)),
		),
		tool_spec(
			"bolt11_claim_for_hash",
			"Manually claim a BOLT11 payment for a specific payment hash",
			schema::bolt11_claim_for_hash_schema,
			|client, args| Box::pin(handlers::handle_bolt11_claim_for_hash(client, args)),
		),
		tool_spec(
			"bolt11_fail_for_hash",
			"Manually fail a BOLT11 payment for a specific payment hash",
			schema::bolt11_fail_for_hash_schema,
			|client, args| Box::pin(handlers::handle_bolt11_fail_for_hash(client, args)),
		),
		tool_spec(
			"bolt11_receive_via_jit_channel",
			"Create a BOLT11 Lightning invoice to receive via an LSPS2 JIT channel",
			schema::bolt11_receive_via_jit_channel_schema,
			|client, args| Box::pin(handlers::handle_bolt11_receive_via_jit_channel(client, args)),
		),
		tool_spec(
			"bolt11_receive_variable_amount_via_jit_channel",
			"Create a variable-amount BOLT11 Lightning invoice to receive via an LSPS2 JIT channel",
			schema::bolt11_receive_variable_amount_via_jit_channel_schema,
			|client, args| {
				Box::pin(handlers::handle_bolt11_receive_variable_amount_via_jit_channel(
					client, args,
				))
			},
		),
		tool_spec(
			"bolt11_send",
			"Pay a BOLT11 Lightning invoice",
			schema::bolt11_send_schema,
			|client, args| Box::pin(handlers::handle_bolt11_send(client, args)),
		),
		tool_spec(
			"bolt12_receive",
			"Create a BOLT12 offer for receiving Lightning payments",
			schema::bolt12_receive_schema,
			|client, args| Box::pin(handlers::handle_bolt12_receive(client, args)),
		),
		tool_spec(
			"bolt12_send",
			"Pay a BOLT12 Lightning offer",
			schema::bolt12_send_schema,
			|client, args| Box::pin(handlers::handle_bolt12_send(client, args)),
		),
		tool_spec(
			"spontaneous_send",
			"Send a spontaneous (keysend) payment to a Lightning node",
			schema::spontaneous_send_schema,
			|client, args| Box::pin(handlers::handle_spontaneous_send(client, args)),
		),
		tool_spec(
			"unified_send",
			"Send a payment given a BIP 21 URI or BIP 353 Human-Readable Name",
			schema::unified_send_schema,
			|client, args| Box::pin(handlers::handle_unified_send(client, args)),
		),
		tool_spec(
			"open_channel",
			"Open a new Lightning channel with a remote node",
			schema::open_channel_schema,
			|client, args| Box::pin(handlers::handle_open_channel(client, args)),
		),
		tool_spec(
			"splice_in",
			"Increase a channel's balance by splicing in on-chain funds",
			schema::splice_in_schema,
			|client, args| Box::pin(handlers::handle_splice_in(client, args)),
		),
		tool_spec(
			"splice_out",
			"Decrease a channel's balance by splicing out to on-chain",
			schema::splice_out_schema,
			|client, args| Box::pin(handlers::handle_splice_out(client, args)),
		),
		tool_spec(
			"close_channel",
			"Cooperatively close a Lightning channel",
			schema::close_channel_schema,
			|client, args| Box::pin(handlers::handle_close_channel(client, args)),
		),
		tool_spec(
			"force_close_channel",
			"Force close a Lightning channel unilaterally",
			schema::force_close_channel_schema,
			|client, args| Box::pin(handlers::handle_force_close_channel(client, args)),
		),
		tool_spec(
			"list_channels",
			"List all known Lightning channels",
			schema::list_channels_schema,
			|client, args| Box::pin(handlers::handle_list_channels(client, args)),
		),
		tool_spec(
			"update_channel_config",
			"Update forwarding fees and CLTV delta for a channel",
			schema::update_channel_config_schema,
			|client, args| Box::pin(handlers::handle_update_channel_config(client, args)),
		),
		tool_spec(
			"list_payments",
			"List all payments (supports pagination via page_token)",
			schema::list_payments_schema,
			|client, args| Box::pin(handlers::handle_list_payments(client, args)),
		),
		tool_spec(
			"get_payment_details",
			"Get details of a specific payment by its ID",
			schema::get_payment_details_schema,
			|client, args| Box::pin(handlers::handle_get_payment_details(client, args)),
		),
		tool_spec(
			"list_forwarded_payments",
			"List all forwarded payments (supports pagination via page_token)",
			schema::list_forwarded_payments_schema,
			|client, args| Box::pin(handlers::handle_list_forwarded_payments(client, args)),
		),
		tool_spec(
			"connect_peer",
			"Connect to a Lightning peer without opening a channel",
			schema::connect_peer_schema,
			|client, args| Box::pin(handlers::handle_connect_peer(client, args)),
		),
		tool_spec(
			"disconnect_peer",
			"Disconnect from a Lightning peer",
			schema::disconnect_peer_schema,
			|client, args| Box::pin(handlers::handle_disconnect_peer(client, args)),
		),
		tool_spec(
			"list_peers",
			"List all known Lightning peers",
			schema::list_peers_schema,
			|client, args| Box::pin(handlers::handle_list_peers(client, args)),
		),
		tool_spec(
			"decode_invoice",
			"Decode a BOLT11 invoice and return its parsed fields",
			schema::decode_invoice_schema,
			|client, args| Box::pin(handlers::handle_decode_invoice(client, args)),
		),
		tool_spec(
			"decode_offer",
			"Decode a BOLT12 offer and return its parsed fields",
			schema::decode_offer_schema,
			|client, args| Box::pin(handlers::handle_decode_offer(client, args)),
		),
		tool_spec(
			"sign_message",
			"Sign a message with the node's secret key",
			schema::sign_message_schema,
			|client, args| Box::pin(handlers::handle_sign_message(client, args)),
		),
		tool_spec(
			"verify_signature",
			"Verify a signature against a message and public key",
			schema::verify_signature_schema,
			|client, args| Box::pin(handlers::handle_verify_signature(client, args)),
		),
		tool_spec(
			"export_pathfinding_scores",
			"Export the pathfinding scores used by the Lightning router",
			schema::export_pathfinding_scores_schema,
			|client, args| Box::pin(handlers::handle_export_pathfinding_scores(client, args)),
		),
		tool_spec(
			"graph_list_channels",
			"List all known short channel IDs in the network graph",
			schema::graph_list_channels_schema,
			|client, args| Box::pin(handlers::handle_graph_list_channels(client, args)),
		),
		tool_spec(
			"graph_get_channel",
			"Get channel information from the network graph by short channel ID",
			schema::graph_get_channel_schema,
			|client, args| Box::pin(handlers::handle_graph_get_channel(client, args)),
		),
		tool_spec(
			"graph_list_nodes",
			"List all known node IDs in the network graph",
			schema::graph_list_nodes_schema,
			|client, args| Box::pin(handlers::handle_graph_list_nodes(client, args)),
		),
		tool_spec(
			"graph_get_node",
			"Get node information from the network graph by node ID",
			schema::graph_get_node_schema,
			|client, args| Box::pin(handlers::handle_graph_get_node(client, args)),
		),
	];

	let mut definitions = Vec::with_capacity(tools.len());
	let mut handlers = HashMap::with_capacity(tools.len());
	for spec in tools {
		definitions.push(ToolDefinition {
			name: spec.name.to_string(),
			description: spec.description.to_string(),
			input_schema: (spec.input_schema)(),
		});
		handlers.insert(spec.name, spec.handler);
	}

	ToolRegistry { definitions, handlers }
}
