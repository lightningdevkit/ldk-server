// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use hex_conservative::DisplayHex;
use ldk_server_client::client::LdkServerClient;
use ldk_server_client::ldk_server_grpc::api::{
	Bolt11ClaimForHashRequest, Bolt11FailForHashRequest, Bolt11ReceiveForHashRequest,
	Bolt11ReceiveRequest, Bolt11ReceiveVariableAmountViaJitChannelRequest,
	Bolt11ReceiveViaJitChannelRequest, Bolt11SendRequest, Bolt12ReceiveRequest, Bolt12SendRequest,
	CloseChannelRequest, ConnectPeerRequest, DecodeInvoiceRequest, DecodeOfferRequest,
	DisconnectPeerRequest, ExportPathfindingScoresRequest, ForceCloseChannelRequest,
	GetBalancesRequest, GetNodeInfoRequest, GetPaymentDetailsRequest, GraphGetChannelRequest,
	GraphGetNodeRequest, GraphListChannelsRequest, GraphListNodesRequest, ListChannelsRequest,
	ListForwardedPaymentsRequest, ListPaymentsRequest, ListPeersRequest, OnchainReceiveRequest,
	OnchainSendRequest, OpenChannelRequest, SignMessageRequest, SpliceInRequest, SpliceOutRequest,
	SpontaneousSendRequest, UnifiedSendRequest, UpdateChannelConfigRequest, VerifySignatureRequest,
};
use ldk_server_client::ldk_server_grpc::types::RouteParametersConfig;
use ldk_server_client::{
	DEFAULT_EXPIRY_SECS, DEFAULT_MAX_CHANNEL_SATURATION_POWER_OF_HALF, DEFAULT_MAX_PATH_COUNT,
	DEFAULT_MAX_TOTAL_CLTV_EXPIRY_DELTA,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

use crate::protocol::McpError;

fn parse_request<T: DeserializeOwned>(args: Value) -> Result<T, McpError> {
	serde_json::from_value(args).map_err(|e| McpError::invalid_params(e.to_string()))
}

fn serialize_response<T: Serialize>(response: T) -> Result<Value, McpError> {
	serde_json::to_value(response)
		.map_err(|e| McpError::internal(format!("Failed to serialize response: {e}")))
}

#[derive(Default)]
struct RouteParameterDefaults {
	max_total_cltv_expiry_delta: bool,
	max_path_count: bool,
	max_channel_saturation_power_of_half: bool,
}

impl RouteParameterDefaults {
	fn from_args(args: &Value) -> Option<Self> {
		let route_parameters = args.get("route_parameters")?.as_object()?;
		Some(Self {
			max_total_cltv_expiry_delta: !route_parameters
				.contains_key("max_total_cltv_expiry_delta"),
			max_path_count: !route_parameters.contains_key("max_path_count"),
			max_channel_saturation_power_of_half: !route_parameters
				.contains_key("max_channel_saturation_power_of_half"),
		})
	}

	fn apply(self, route_parameters: &mut RouteParametersConfig) {
		if self.max_total_cltv_expiry_delta {
			route_parameters.max_total_cltv_expiry_delta = DEFAULT_MAX_TOTAL_CLTV_EXPIRY_DELTA;
		}
		if self.max_path_count {
			route_parameters.max_path_count = DEFAULT_MAX_PATH_COUNT;
		}
		if self.max_channel_saturation_power_of_half {
			route_parameters.max_channel_saturation_power_of_half =
				DEFAULT_MAX_CHANNEL_SATURATION_POWER_OF_HALF;
		}
	}
}

fn parse_request_with_route_parameters<T, F>(
	args: Value, route_parameters: F,
) -> Result<T, McpError>
where
	T: DeserializeOwned,
	F: FnOnce(&mut T) -> &mut Option<RouteParametersConfig>,
{
	let route_defaults = RouteParameterDefaults::from_args(&args);
	let mut request = parse_request(args)?;
	if let Some(route_defaults) = route_defaults {
		if let Some(route_parameters) = route_parameters(&mut request).as_mut() {
			route_defaults.apply(route_parameters);
		}
	}
	Ok(request)
}

pub async fn handle_get_node_info(
	client: &LdkServerClient, _args: Value,
) -> Result<Value, McpError> {
	let response = client.get_node_info(GetNodeInfoRequest {}).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_get_balances(
	client: &LdkServerClient, _args: Value,
) -> Result<Value, McpError> {
	let response = client.get_balances(GetBalancesRequest {}).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_onchain_receive(
	client: &LdkServerClient, _args: Value,
) -> Result<Value, McpError> {
	let response =
		client.onchain_receive(OnchainReceiveRequest {}).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_onchain_send(client: &LdkServerClient, args: Value) -> Result<Value, McpError> {
	let request: OnchainSendRequest = parse_request(args)?;
	let response = client.onchain_send(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_bolt11_receive(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let mut request: Bolt11ReceiveRequest = parse_request(args)?;
	if request.expiry_secs == 0 {
		request.expiry_secs = DEFAULT_EXPIRY_SECS;
	}
	let response = client.bolt11_receive(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_bolt11_receive_for_hash(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let mut request: Bolt11ReceiveForHashRequest = parse_request(args)?;
	if request.expiry_secs == 0 {
		request.expiry_secs = DEFAULT_EXPIRY_SECS;
	}
	let response = client.bolt11_receive_for_hash(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_bolt11_claim_for_hash(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: Bolt11ClaimForHashRequest = parse_request(args)?;
	let response = client.bolt11_claim_for_hash(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_bolt11_fail_for_hash(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: Bolt11FailForHashRequest = parse_request(args)?;
	let response = client.bolt11_fail_for_hash(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_bolt11_receive_via_jit_channel(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let mut request: Bolt11ReceiveViaJitChannelRequest = parse_request(args)?;
	if request.expiry_secs == 0 {
		request.expiry_secs = DEFAULT_EXPIRY_SECS;
	}
	let response = client.bolt11_receive_via_jit_channel(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_bolt11_receive_variable_amount_via_jit_channel(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let mut request: Bolt11ReceiveVariableAmountViaJitChannelRequest = parse_request(args)?;
	if request.expiry_secs == 0 {
		request.expiry_secs = DEFAULT_EXPIRY_SECS;
	}
	let response = client
		.bolt11_receive_variable_amount_via_jit_channel(request)
		.await
		.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_bolt11_send(client: &LdkServerClient, args: Value) -> Result<Value, McpError> {
	let request: Bolt11SendRequest =
		parse_request_with_route_parameters(args, |request: &mut Bolt11SendRequest| {
			&mut request.route_parameters
		})?;
	let response = client.bolt11_send(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_bolt12_receive(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: Bolt12ReceiveRequest = parse_request(args)?;
	let response = client.bolt12_receive(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_bolt12_send(client: &LdkServerClient, args: Value) -> Result<Value, McpError> {
	let request: Bolt12SendRequest =
		parse_request_with_route_parameters(args, |request: &mut Bolt12SendRequest| {
			&mut request.route_parameters
		})?;
	let response = client.bolt12_send(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_spontaneous_send(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: SpontaneousSendRequest =
		parse_request_with_route_parameters(args, |request: &mut SpontaneousSendRequest| {
			&mut request.route_parameters
		})?;
	let response = client.spontaneous_send(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_unified_send(client: &LdkServerClient, args: Value) -> Result<Value, McpError> {
	let request: UnifiedSendRequest =
		parse_request_with_route_parameters(args, |request: &mut UnifiedSendRequest| {
			&mut request.route_parameters
		})?;
	let response = client.unified_send(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_open_channel(client: &LdkServerClient, args: Value) -> Result<Value, McpError> {
	let request: OpenChannelRequest = parse_request(args)?;
	let response = client.open_channel(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_splice_in(client: &LdkServerClient, args: Value) -> Result<Value, McpError> {
	let request: SpliceInRequest = parse_request(args)?;
	let response = client.splice_in(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_splice_out(client: &LdkServerClient, args: Value) -> Result<Value, McpError> {
	let request: SpliceOutRequest = parse_request(args)?;
	let response = client.splice_out(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_close_channel(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: CloseChannelRequest = parse_request(args)?;
	let response = client.close_channel(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_force_close_channel(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: ForceCloseChannelRequest = parse_request(args)?;
	let response = client.force_close_channel(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_list_channels(
	client: &LdkServerClient, _args: Value,
) -> Result<Value, McpError> {
	let response = client.list_channels(ListChannelsRequest {}).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_update_channel_config(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: UpdateChannelConfigRequest = parse_request(args)?;
	let response = client.update_channel_config(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_list_payments(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: ListPaymentsRequest = parse_request(args)?;
	let response = client.list_payments(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_get_payment_details(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: GetPaymentDetailsRequest = parse_request(args)?;
	let response = client.get_payment_details(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_list_forwarded_payments(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: ListForwardedPaymentsRequest = parse_request(args)?;
	let response = client.list_forwarded_payments(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_connect_peer(client: &LdkServerClient, args: Value) -> Result<Value, McpError> {
	let request: ConnectPeerRequest = parse_request(args)?;
	let response = client.connect_peer(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_disconnect_peer(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: DisconnectPeerRequest = parse_request(args)?;
	let response = client.disconnect_peer(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_list_peers(client: &LdkServerClient, _args: Value) -> Result<Value, McpError> {
	let response = client.list_peers(ListPeersRequest {}).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_decode_invoice(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: DecodeInvoiceRequest = parse_request(args)?;
	let response = client.decode_invoice(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_decode_offer(client: &LdkServerClient, args: Value) -> Result<Value, McpError> {
	let request: DecodeOfferRequest = parse_request(args)?;
	let response = client.decode_offer(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

// The proto `message` field is `bytes`, whose Deserialize impl expects a numeric array, but MCP
// clients naturally pass a UTF-8 string. We deserialize into a local args struct first and then
// build the proto request from it.
#[derive(Deserialize)]
struct SignMessageArgs {
	message: String,
}

pub async fn handle_sign_message(client: &LdkServerClient, args: Value) -> Result<Value, McpError> {
	let SignMessageArgs { message } = parse_request(args)?;
	let request = SignMessageRequest { message: message.into_bytes().into() };
	let response = client.sign_message(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

#[derive(Deserialize)]
struct VerifySignatureArgs {
	message: String,
	signature: String,
	public_key: String,
}

pub async fn handle_verify_signature(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let VerifySignatureArgs { message, signature, public_key } = parse_request(args)?;
	let request =
		VerifySignatureRequest { message: message.into_bytes().into(), signature, public_key };
	let response = client.verify_signature(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_export_pathfinding_scores(
	client: &LdkServerClient, _args: Value,
) -> Result<Value, McpError> {
	let response = client
		.export_pathfinding_scores(ExportPathfindingScoresRequest {})
		.await
		.map_err(McpError::from)?;
	Ok(json!({ "pathfinding_scores": response.scores.to_lower_hex_string() }))
}

pub async fn handle_graph_list_channels(
	client: &LdkServerClient, _args: Value,
) -> Result<Value, McpError> {
	let response =
		client.graph_list_channels(GraphListChannelsRequest {}).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_graph_get_channel(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: GraphGetChannelRequest = parse_request(args)?;
	let response = client.graph_get_channel(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_graph_list_nodes(
	client: &LdkServerClient, _args: Value,
) -> Result<Value, McpError> {
	let response =
		client.graph_list_nodes(GraphListNodesRequest {}).await.map_err(McpError::from)?;
	serialize_response(response)
}

pub async fn handle_graph_get_node(
	client: &LdkServerClient, args: Value,
) -> Result<Value, McpError> {
	let request: GraphGetNodeRequest = parse_request(args)?;
	let response = client.graph_get_node(request).await.map_err(McpError::from)?;
	serialize_response(response)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_request_with_route_parameters_fills_missing_defaults() {
		let request: Bolt11SendRequest = parse_request_with_route_parameters(
			json!({
				"invoice": "lnbc1example",
				"route_parameters": {
					"max_path_count": 3
				}
			}),
			|request: &mut Bolt11SendRequest| &mut request.route_parameters,
		)
		.unwrap();

		let route_parameters = request.route_parameters.unwrap();
		assert_eq!(
			route_parameters.max_total_cltv_expiry_delta,
			DEFAULT_MAX_TOTAL_CLTV_EXPIRY_DELTA
		);
		assert_eq!(route_parameters.max_path_count, 3);
		assert_eq!(
			route_parameters.max_channel_saturation_power_of_half,
			DEFAULT_MAX_CHANNEL_SATURATION_POWER_OF_HALF
		);
	}

	#[test]
	fn parse_request_with_route_parameters_preserves_explicit_values() {
		let request: UnifiedSendRequest = parse_request_with_route_parameters(
			json!({
				"uri": "bitcoin:tb1qexample?amount=0.001",
				"route_parameters": {
					"max_total_cltv_expiry_delta": 0,
					"max_path_count": 1,
					"max_channel_saturation_power_of_half": 4
				}
			}),
			|request: &mut UnifiedSendRequest| &mut request.route_parameters,
		)
		.unwrap();

		let route_parameters = request.route_parameters.unwrap();
		assert_eq!(route_parameters.max_total_cltv_expiry_delta, 0);
		assert_eq!(route_parameters.max_path_count, 1);
		assert_eq!(route_parameters.max_channel_saturation_power_of_half, 4);
	}
}
