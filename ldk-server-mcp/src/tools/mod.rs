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
use ldk_server_client::ldk_server_grpc::api::MacaroonInfo;
use ldk_server_client::ldk_server_grpc::endpoints::{
	BOLT11_CLAIM_FOR_ID_PATH, BOLT11_FAIL_FOR_ID_PATH, BOLT11_RECEIVE_FOR_HASH_PATH,
	BOLT11_RECEIVE_PATH, BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_FOR_HASH_PATH,
	BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH,
	BOLT11_RECEIVE_VIA_JIT_CHANNEL_FOR_HASH_PATH, BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH,
	BOLT11_SEND_PATH, BOLT11_SEND_UNDERPAYING_PATH, BOLT12_CREATE_PAYER_PROOF_PATH,
	BOLT12_RECEIVE_PATH, BOLT12_RECEIVE_REFUND_PATH, BOLT12_SEND_PATH, BOLT12_SEND_REFUND_PATH,
	BUMP_CHANNEL_FUNDING_FEE_PATH, CLOSE_CHANNEL_PATH, CONNECT_PEER_PATH, CREATE_MACAROON_PATH,
	DECODE_INVOICE_PATH, DECODE_OFFER_PATH, DISCONNECT_PEER_PATH, EXPORT_PATHFINDING_SCORES_PATH,
	FORCE_CLOSE_CHANNEL_PATH, GET_BALANCES_PATH, GET_CHANNEL_FORWARDING_STATS_PATH,
	GET_FORWARDED_PAYMENT_DETAILS_PATH, GET_FORWARDED_PAYMENT_TRACKING_MODE_PATH,
	GET_NODE_INFO_PATH, GET_PAYMENT_DETAILS_PATH, GET_PERMISSIONS_PATH, GRAPH_GET_CHANNEL_PATH,
	GRAPH_GET_NODE_PATH, GRAPH_LIST_CHANNELS_PATH, GRAPH_LIST_NODES_PATH, LIST_CHANNELS_PATH,
	LIST_CHANNEL_FORWARDING_STATS_PATH, LIST_CHANNEL_PAIR_FORWARDING_STATS_PATH,
	LIST_FORWARDED_PAYMENTS_PATH, LIST_MACAROONS_PATH, LIST_PAYMENTS_PATH, LIST_PEERS_PATH,
	ONCHAIN_BUMP_FEE_PATH, ONCHAIN_RECEIVE_PATH, ONCHAIN_SEND_PATH, OPEN_CHANNEL_PATH,
	REVOKE_MACAROON_PATH, SIGN_MESSAGE_PATH, SPLICE_IN_PATH, SPLICE_OUT_PATH,
	SPONTANEOUS_SEND_PATH, UNIFIED_SEND_PATH, UPDATE_CHANNEL_CONFIG_PATH, VERIFY_SIGNATURE_PATH,
};
use ldk_server_client::ldk_server_grpc::permissions::{
	method_authorization, MethodAuthorization, ADMIN_PERMISSION,
};
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
	rpc_methods: HashMap<&'static str, &'static str>,
	handlers: HashMap<&'static str, ToolHandler>,
}

struct ToolSpec {
	name: &'static str,
	/// The RPC this tool calls, used to decide whether a macaroon may see it.
	rpc_method: &'static str,
	description: &'static str,
	input_schema: fn() -> Value,
	handler: ToolHandler,
}

fn tool_spec(
	name: &'static str, rpc_method: &'static str, description: &'static str,
	input_schema: fn() -> Value, handler: ToolHandler,
) -> ToolSpec {
	ToolSpec { name, rpc_method, description, input_schema, handler }
}

impl ToolRegistry {
	pub fn list_tools(&self) -> &[ToolDefinition] {
		&self.definitions
	}

	/// Stops listing tools the given macaroon is not allowed to call.
	///
	/// `macaroon` is the caller's own details as returned by `get_permissions`, with its
	/// permissions already narrowed by its caveats. Hidden tools stay callable so that a stale
	/// list still gets the server's permission error rather than an unknown tool error.
	pub fn retain_allowed_tools(&mut self, macaroon: &MacaroonInfo) {
		let rpc_methods = &self.rpc_methods;
		self.definitions.retain(|tool| {
			rpc_methods
				.get(tool.name.as_str())
				.is_some_and(|method| macaroon_allows(macaroon, method))
		});
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

/// Mirrors the server's checks for whether `macaroon` may call `method`.
fn macaroon_allows(macaroon: &MacaroonInfo, method: &str) -> bool {
	match method_authorization(method) {
		// The server lets any valid macaroon inspect itself, even with a `method` caveat.
		MethodAuthorization::AuthenticatedOnly => true,
		MethodAuthorization::Unknown => false,
		MethodAuthorization::Permission(required) => {
			let has_permission =
				macaroon.permissions.iter().any(|p| p == ADMIN_PERMISSION || p == required);
			let method_allowed = macaroon
				.caveats
				.iter()
				.all(|caveat| caveat.strip_prefix("method = ").is_none_or(|m| m == method));
			has_permission && method_allowed
		},
	}
}

pub fn build_tool_registry() -> ToolRegistry {
	let tools = vec![
		tool_spec(
			"get_permissions",
			GET_PERMISSIONS_PATH,
			"Show the current macaroon's permissions and restrictions",
			schema::get_permissions_schema,
			|client, args| Box::pin(handlers::handle_get_permissions(client, args)),
		),
		tool_spec(
			"revoke_macaroon",
			REVOKE_MACAROON_PATH,
			"Revoke a macaroon for new requests",
			schema::revoke_macaroon_schema,
			|client, args| Box::pin(handlers::handle_revoke_macaroon(client, args)),
		),
		tool_spec(
			"list_macaroons",
			LIST_MACAROONS_PATH,
			"List macaroon IDs, names, permissions, and restrictions",
			schema::list_macaroons_schema,
			|client, args| Box::pin(handlers::handle_list_macaroons(client, args)),
		),
		tool_spec(
			"create_macaroon",
			CREATE_MACAROON_PATH,
			"Create a macaroon with chosen permissions and return its private token",
			schema::create_macaroon_schema,
			|client, args| Box::pin(handlers::handle_create_macaroon(client, args)),
		),
		tool_spec(
			"get_node_info",
			GET_NODE_INFO_PATH,
			"Retrieve node info including node_id, sync status, and best block",
			schema::get_node_info_schema,
			|client, args| Box::pin(handlers::handle_get_node_info(client, args)),
		),
		tool_spec(
			"get_balances",
			GET_BALANCES_PATH,
			"Retrieve an overview of all known balances (on-chain and Lightning)",
			schema::get_balances_schema,
			|client, args| Box::pin(handlers::handle_get_balances(client, args)),
		),
		tool_spec(
			"onchain_receive",
			ONCHAIN_RECEIVE_PATH,
			"Generate a new on-chain Bitcoin funding address",
			schema::onchain_receive_schema,
			|client, args| Box::pin(handlers::handle_onchain_receive(client, args)),
		),
		tool_spec(
			"onchain_bump_fee",
			ONCHAIN_BUMP_FEE_PATH,
			"Replace an unconfirmed outbound on-chain payment using RBF. Funding payments are not eligible. Returns the replacement transaction ID",
			schema::onchain_bump_fee_schema,
			|client, args| Box::pin(handlers::handle_onchain_bump_fee(client, args)),
		),
		tool_spec(
			"onchain_send",
			ONCHAIN_SEND_PATH,
			"Send an on-chain Bitcoin payment to an address",
			schema::onchain_send_schema,
			|client, args| Box::pin(handlers::handle_onchain_send(client, args)),
		),
		tool_spec(
			"bolt11_receive",
			BOLT11_RECEIVE_PATH,
			"Create a BOLT11 Lightning invoice to receive a payment",
			schema::bolt11_receive_schema,
			|client, args| Box::pin(handlers::handle_bolt11_receive(client, args)),
		),
		tool_spec(
			"bolt11_receive_for_hash",
			BOLT11_RECEIVE_FOR_HASH_PATH,
			"Create a BOLT11 Lightning invoice for a specific payment hash",
			schema::bolt11_receive_for_hash_schema,
			|client, args| Box::pin(handlers::handle_bolt11_receive_for_hash(client, args)),
		),
		tool_spec(
			"bolt11_claim_for_id",
			BOLT11_CLAIM_FOR_ID_PATH,
			"Manually claim a BOLT11 payment for a specific payment ID",
			schema::bolt11_claim_for_id_schema,
			|client, args| Box::pin(handlers::handle_bolt11_claim_for_id(client, args)),
		),
		tool_spec(
			"bolt11_fail_for_id",
			BOLT11_FAIL_FOR_ID_PATH,
			"Manually fail a BOLT11 payment for a specific payment ID",
			schema::bolt11_fail_for_id_schema,
			|client, args| Box::pin(handlers::handle_bolt11_fail_for_id(client, args)),
		),
		tool_spec(
			"bolt11_receive_via_jit_channel",
			BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH,
			"Create a BOLT11 Lightning invoice to receive via an LSPS2 JIT channel",
			schema::bolt11_receive_via_jit_channel_schema,
			|client, args| Box::pin(handlers::handle_bolt11_receive_via_jit_channel(client, args)),
		),
		tool_spec(
			"bolt11_receive_variable_amount_via_jit_channel",
			BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH,
			"Create a variable-amount BOLT11 Lightning invoice to receive via an LSPS2 JIT channel",
			schema::bolt11_receive_variable_amount_via_jit_channel_schema,
			|client, args| {
				Box::pin(handlers::handle_bolt11_receive_variable_amount_via_jit_channel(
					client, args,
				))
			},
		),
		tool_spec(
			"bolt11_receive_via_jit_channel_for_hash",
			BOLT11_RECEIVE_VIA_JIT_CHANNEL_FOR_HASH_PATH,
			"Create a BOLT11 Lightning invoice to receive via an LSPS2 JIT channel for a given payment hash",
			schema::bolt11_receive_via_jit_channel_for_hash_schema,
			|client, args| Box::pin(handlers::handle_bolt11_receive_via_jit_channel_for_hash(client, args)),
		),
		tool_spec(
			"bolt11_receive_variable_amount_via_jit_channel_for_hash",
			BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_FOR_HASH_PATH,
			"Create a variable-amount BOLT11 Lightning invoice to receive via an LSPS2 JIT channel for a given payment hash",
			schema::bolt11_receive_variable_amount_via_jit_channel_for_hash_schema,
			|client, args| {
				Box::pin(handlers::handle_bolt11_receive_variable_amount_via_jit_channel_for_hash(
					client, args,
				))
			},
		),
		tool_spec(
			"bolt11_send",
			BOLT11_SEND_PATH,
			"Pay a BOLT11 Lightning invoice",
			schema::bolt11_send_schema,
			|client, args| Box::pin(handlers::handle_bolt11_send(client, args)),
		),
		tool_spec(
			"bolt11_send_underpaying",
			BOLT11_SEND_UNDERPAYING_PATH,
			"Send part of a fixed-amount BOLT11 invoice. Other nodes must send partial payments for the same invoice until the combined amount equals the invoice amount",
			schema::bolt11_send_underpaying_schema,
			|client, args| Box::pin(handlers::handle_bolt11_send_underpaying(client, args)),
		),
		tool_spec(
			"bolt12_receive",
			BOLT12_RECEIVE_PATH,
			"Create a reusable BOLT12 offer for receiving Lightning payments",
			schema::bolt12_receive_schema,
			|client, args| Box::pin(handlers::handle_bolt12_receive(client, args)),
		),
		tool_spec(
			"bolt12_send",
			BOLT12_SEND_PATH,
			"Pay a BOLT12 Lightning offer",
			schema::bolt12_send_schema,
			|client, args| Box::pin(handlers::handle_bolt12_send(client, args)),
		),
		tool_spec(
			"bolt12_send_refund",
			BOLT12_SEND_REFUND_PATH,
			"Create a BOLT12 refund that this node will pay",
			schema::bolt12_send_refund_schema,
			|client, args| Box::pin(handlers::handle_bolt12_send_refund(client, args)),
		),
		tool_spec(
			"bolt12_receive_refund",
			BOLT12_RECEIVE_REFUND_PATH,
			"Request an incoming payment for a BOLT12 refund",
			schema::bolt12_receive_refund_schema,
			|client, args| Box::pin(handlers::handle_bolt12_receive_refund(client, args)),
		),
		tool_spec(
			"bolt12_create_payer_proof",
			BOLT12_CREATE_PAYER_PROOF_PATH,
			"Create a BOLT12 payer proof for a payment this node made",
			schema::bolt12_create_payer_proof_schema,
			|client, args| Box::pin(handlers::handle_bolt12_create_payer_proof(client, args)),
		),
		tool_spec(
			"spontaneous_send",
			SPONTANEOUS_SEND_PATH,
			"Send a spontaneous (keysend) payment to a Lightning node",
			schema::spontaneous_send_schema,
			|client, args| Box::pin(handlers::handle_spontaneous_send(client, args)),
		),
		tool_spec(
			"unified_send",
			UNIFIED_SEND_PATH,
			"Send a payment given a BIP 21 URI or BIP 353 Human-Readable Name",
			schema::unified_send_schema,
			|client, args| Box::pin(handlers::handle_unified_send(client, args)),
		),
		tool_spec(
			"open_channel",
			OPEN_CHANNEL_PATH,
			"Open a new Lightning channel with a remote node",
			schema::open_channel_schema,
			|client, args| Box::pin(handlers::handle_open_channel(client, args)),
		),
		tool_spec(
			"splice_in",
			SPLICE_IN_PATH,
			"Increase a channel's balance by splicing in on-chain funds",
			schema::splice_in_schema,
			|client, args| Box::pin(handlers::handle_splice_in(client, args)),
		),
		tool_spec(
			"splice_out",
			SPLICE_OUT_PATH,
			"Decrease a channel's balance by splicing out to on-chain",
			schema::splice_out_schema,
			|client, args| Box::pin(handlers::handle_splice_out(client, args)),
		),
		tool_spec(
			"bump_channel_funding_fee",
			BUMP_CHANNEL_FUNDING_FEE_PATH,
			"Bump a pending splice fee, preserving its amount and destination. No general channel-opening fee bumping or caller-selected fee rate. Returns empty success when initiated",
			schema::bump_channel_funding_fee_schema,
			|client, args| Box::pin(handlers::handle_bump_channel_funding_fee(client, args)),
		),
		tool_spec(
			"close_channel",
			CLOSE_CHANNEL_PATH,
			"Cooperatively close a Lightning channel",
			schema::close_channel_schema,
			|client, args| Box::pin(handlers::handle_close_channel(client, args)),
		),
		tool_spec(
			"force_close_channel",
			FORCE_CLOSE_CHANNEL_PATH,
			"Force close a Lightning channel unilaterally",
			schema::force_close_channel_schema,
			|client, args| Box::pin(handlers::handle_force_close_channel(client, args)),
		),
		tool_spec(
			"list_channels",
			LIST_CHANNELS_PATH,
			"List all known Lightning channels",
			schema::list_channels_schema,
			|client, args| Box::pin(handlers::handle_list_channels(client, args)),
		),
		tool_spec(
			"update_channel_config",
			UPDATE_CHANNEL_CONFIG_PATH,
			"Update forwarding fees and CLTV delta for a channel",
			schema::update_channel_config_schema,
			|client, args| Box::pin(handlers::handle_update_channel_config(client, args)),
		),
		tool_spec(
			"list_payments",
			LIST_PAYMENTS_PATH,
			"List all payments (supports pagination via page_token)",
			schema::list_payments_schema,
			|client, args| Box::pin(handlers::handle_list_payments(client, args)),
		),
		tool_spec(
			"get_payment_details",
			GET_PAYMENT_DETAILS_PATH,
			"Get details of a specific payment by its ID",
			schema::get_payment_details_schema,
			|client, args| Box::pin(handlers::handle_get_payment_details(client, args)),
		),
		tool_spec(
			"get_forwarded_payment_details",
			GET_FORWARDED_PAYMENT_DETAILS_PATH,
			"Get a stored forwarded payment by its ID",
			schema::get_forwarded_payment_details_schema,
			|client, args| Box::pin(handlers::handle_get_forwarded_payment_details(client, args)),
		),
		tool_spec(
			"get_forwarded_payment_tracking_mode",
			GET_FORWARDED_PAYMENT_TRACKING_MODE_PATH,
			"Get the configured forwarding history tracking mode",
			schema::get_forwarded_payment_tracking_mode_schema,
			|client, args| Box::pin(handlers::handle_get_forwarded_payment_tracking_mode(client, args)),
		),
		tool_spec(
			"get_channel_forwarding_stats",
			GET_CHANNEL_FORWARDING_STATS_PATH,
			"Get forwarding statistics for a channel",
			schema::get_channel_forwarding_stats_schema,
			|client, args| Box::pin(handlers::handle_get_channel_forwarding_stats(client, args)),
		),
		tool_spec(
			"list_channel_forwarding_stats",
			LIST_CHANNEL_FORWARDING_STATS_PATH,
			"List channel forwarding statistics (paginated)",
			schema::list_channel_forwarding_stats_schema,
			|client, args| Box::pin(handlers::handle_list_channel_forwarding_stats(client, args)),
		),
		tool_spec(
			"list_channel_pair_forwarding_stats",
			LIST_CHANNEL_PAIR_FORWARDING_STATS_PATH,
			"List channel-pair forwarding statistics (paginated)",
			schema::list_channel_pair_forwarding_stats_schema,
			|client, args| Box::pin(handlers::handle_list_channel_pair_forwarding_stats(client, args)),
		),
		tool_spec(
			"list_forwarded_payments",
			LIST_FORWARDED_PAYMENTS_PATH,
			"Retrieve a paginated list of forwarded payments (use page_token for subsequent pages)",
			schema::list_forwarded_payments_schema,
			|client, args| Box::pin(handlers::handle_list_forwarded_payments(client, args)),
		),
		tool_spec(
			"connect_peer",
			CONNECT_PEER_PATH,
			"Connect to a Lightning peer without opening a channel",
			schema::connect_peer_schema,
			|client, args| Box::pin(handlers::handle_connect_peer(client, args)),
		),
		tool_spec(
			"disconnect_peer",
			DISCONNECT_PEER_PATH,
			"Disconnect from a Lightning peer",
			schema::disconnect_peer_schema,
			|client, args| Box::pin(handlers::handle_disconnect_peer(client, args)),
		),
		tool_spec(
			"list_peers",
			LIST_PEERS_PATH,
			"List all known Lightning peers",
			schema::list_peers_schema,
			|client, args| Box::pin(handlers::handle_list_peers(client, args)),
		),
		tool_spec(
			"decode_invoice",
			DECODE_INVOICE_PATH,
			"Decode a BOLT11 invoice and return its parsed fields",
			schema::decode_invoice_schema,
			|client, args| Box::pin(handlers::handle_decode_invoice(client, args)),
		),
		tool_spec(
			"decode_offer",
			DECODE_OFFER_PATH,
			"Decode a BOLT12 offer and return its parsed fields",
			schema::decode_offer_schema,
			|client, args| Box::pin(handlers::handle_decode_offer(client, args)),
		),
		tool_spec(
			"sign_message",
			SIGN_MESSAGE_PATH,
			"Sign a message with the node's secret key",
			schema::sign_message_schema,
			|client, args| Box::pin(handlers::handle_sign_message(client, args)),
		),
		tool_spec(
			"verify_signature",
			VERIFY_SIGNATURE_PATH,
			"Verify a signature against a message and public key",
			schema::verify_signature_schema,
			|client, args| Box::pin(handlers::handle_verify_signature(client, args)),
		),
		tool_spec(
			"export_pathfinding_scores",
			EXPORT_PATHFINDING_SCORES_PATH,
			"Export the pathfinding scores used by the Lightning router",
			schema::export_pathfinding_scores_schema,
			|client, args| Box::pin(handlers::handle_export_pathfinding_scores(client, args)),
		),
		tool_spec(
			"graph_list_channels",
			GRAPH_LIST_CHANNELS_PATH,
			"List all known short channel IDs in the network graph",
			schema::graph_list_channels_schema,
			|client, args| Box::pin(handlers::handle_graph_list_channels(client, args)),
		),
		tool_spec(
			"graph_get_channel",
			GRAPH_GET_CHANNEL_PATH,
			"Get channel information from the network graph by short channel ID",
			schema::graph_get_channel_schema,
			|client, args| Box::pin(handlers::handle_graph_get_channel(client, args)),
		),
		tool_spec(
			"graph_list_nodes",
			GRAPH_LIST_NODES_PATH,
			"List all known node IDs in the network graph",
			schema::graph_list_nodes_schema,
			|client, args| Box::pin(handlers::handle_graph_list_nodes(client, args)),
		),
		tool_spec(
			"graph_get_node",
			GRAPH_GET_NODE_PATH,
			"Get node information from the network graph by node ID",
			schema::graph_get_node_schema,
			|client, args| Box::pin(handlers::handle_graph_get_node(client, args)),
		),
	];

	let mut definitions = Vec::with_capacity(tools.len());
	let mut rpc_methods = HashMap::with_capacity(tools.len());
	let mut handlers = HashMap::with_capacity(tools.len());
	for spec in tools {
		definitions.push(ToolDefinition {
			name: spec.name.to_string(),
			description: spec.description.to_string(),
			input_schema: (spec.input_schema)(),
		});
		rpc_methods.insert(spec.name, spec.rpc_method);
		handlers.insert(spec.name, spec.handler);
	}

	ToolRegistry { definitions, rpc_methods, handlers }
}

#[cfg(test)]
mod tests {
	use ldk_server_client::ldk_server_grpc::permissions::{
		INVOICE_PERMISSIONS, NODE_READ_PERMISSION, READONLY_PERMISSIONS,
	};

	use super::*;

	fn macaroon(permissions: &[&str], caveats: &[&str]) -> MacaroonInfo {
		MacaroonInfo {
			id: "id".to_string(),
			name: "test".to_string(),
			permissions: permissions.iter().map(|p| p.to_string()).collect(),
			caveats: caveats.iter().map(|c| c.to_string()).collect(),
		}
	}

	fn visible_tools(macaroon: &MacaroonInfo) -> Vec<String> {
		let mut registry = build_tool_registry();
		registry.retain_allowed_tools(macaroon);
		let mut names: Vec<_> = registry.list_tools().iter().map(|t| t.name.clone()).collect();
		names.sort();
		names
	}

	fn tools_requiring(permissions: &[&str]) -> Vec<String> {
		let registry = build_tool_registry();
		let mut names: Vec<_> = registry
			.list_tools()
			.iter()
			.map(|tool| tool.name.clone())
			.filter(|name| match method_authorization(registry.rpc_methods[name.as_str()]) {
				MethodAuthorization::Permission(required) => permissions.contains(&required),
				MethodAuthorization::AuthenticatedOnly => true,
				MethodAuthorization::Unknown => false,
			})
			.collect();
		names.sort();
		names
	}

	#[test]
	fn every_tool_has_a_known_rpc_method() {
		let registry = build_tool_registry();
		assert_eq!(registry.rpc_methods.len(), registry.list_tools().len());
		for tool in registry.list_tools() {
			let method = registry.rpc_methods[tool.name.as_str()];
			assert_ne!(
				method_authorization(method),
				MethodAuthorization::Unknown,
				"Tool {} calls RPC {method}, which has no permission mapping",
				tool.name
			);
		}
		assert_eq!(registry.rpc_methods["get_permissions"], GET_PERMISSIONS_PATH);
		assert_eq!(registry.rpc_methods["bolt11_receive_for_hash"], BOLT11_RECEIVE_FOR_HASH_PATH);
	}

	#[test]
	fn admin_macaroon_sees_every_tool() {
		let mut expected: Vec<_> =
			build_tool_registry().list_tools().iter().map(|t| t.name.clone()).collect();
		expected.sort();
		assert_eq!(visible_tools(&macaroon(&[ADMIN_PERMISSION], &[])), expected);
		let expiring = macaroon(&[ADMIN_PERMISSION], &["time-before = 4102444800"]);
		assert_eq!(visible_tools(&expiring), expected);
	}

	#[test]
	fn restricted_macaroon_sees_only_permitted_tools() {
		assert_eq!(visible_tools(&macaroon(&[], &[])), ["get_permissions"]);
		let reader = visible_tools(&macaroon(&[NODE_READ_PERMISSION], &[]));
		assert_eq!(
			reader,
			["export_pathfinding_scores", "get_balances", "get_node_info", "get_permissions"]
		);
		assert_eq!(
			visible_tools(&macaroon(&READONLY_PERMISSIONS, &[])),
			tools_requiring(&READONLY_PERMISSIONS)
		);
		let invoice = visible_tools(&macaroon(&INVOICE_PERMISSIONS, &[]));
		assert_eq!(invoice, tools_requiring(&INVOICE_PERMISSIONS));
		assert!(invoice.contains(&"bolt11_receive".to_string()));
		for hidden in ["bolt11_send", "onchain_send", "open_channel", "create_macaroon"] {
			assert!(!invoice.contains(&hidden.to_string()), "{hidden} should be hidden");
		}
	}

	#[test]
	fn method_caveats_limit_visible_tools() {
		let limited = macaroon(&[ADMIN_PERMISSION], &["method = GetBalances"]);
		assert_eq!(visible_tools(&limited), ["get_balances", "get_permissions"]);
		// The method must also be covered by the granted permissions.
		let unpermitted = macaroon(&[NODE_READ_PERMISSION], &["method = ListChannels"]);
		assert_eq!(visible_tools(&unpermitted), ["get_permissions"]);
		let conflicting =
			macaroon(&[ADMIN_PERMISSION], &["method = GetBalances", "method = GetNodeInfo"]);
		assert_eq!(visible_tools(&conflicting), ["get_permissions"]);
	}

	#[test]
	fn hidden_tools_remain_callable() {
		let mut registry = build_tool_registry();
		registry.retain_allowed_tools(&macaroon(&[NODE_READ_PERMISSION], &[]));
		assert!(registry.list_tools().iter().all(|tool| tool.name != "bolt11_send"));
		assert!(registry.handlers.contains_key("bolt11_send"));
	}
}
