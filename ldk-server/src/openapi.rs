// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

#![allow(dead_code, unused_imports)]

use ldk_server_json_models::api::*;
use ldk_server_json_models::error::{ErrorCode, ErrorResponse};
use ldk_server_json_models::events::*;
use ldk_server_json_models::types::*;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};

#[derive(OpenApi)]
#[openapi(
	info(
		title = "LDK Server API",
		version = "0.1.0",
		description = "REST API for LDK Server, a Lightning Network node daemon built on LDK."
	),
	paths(
		get_node_info,
		get_balances,
		onchain_receive,
		onchain_send,
		bolt11_receive,
		bolt11_receive_for_hash,
		bolt11_claim_for_hash,
		bolt11_fail_for_hash,
		bolt11_receive_via_jit_channel,
		bolt11_receive_variable_amount_via_jit_channel,
		bolt11_send,
		bolt12_receive,
		bolt12_send,
		spontaneous_send,
		open_channel,
		splice_in,
		splice_out,
		close_channel,
		force_close_channel,
		list_channels,
		update_channel_config,
		get_payment_details,
		list_payments,
		list_forwarded_payments,
		list_peers,
		connect_peer,
		disconnect_peer,
		sign_message,
		verify_signature,
		export_pathfinding_scores,
		unified_send,
		graph_list_channels,
		graph_get_channel,
		graph_list_nodes,
		graph_get_node,
		subscribe,
	),
	components(schemas(
		// API request/response types
		GetNodeInfoRequest, GetNodeInfoResponse,
		GetBalancesRequest, GetBalancesResponse,
		OnchainReceiveRequest, OnchainReceiveResponse,
		OnchainSendRequest, OnchainSendResponse,
		Bolt11ReceiveRequest, Bolt11ReceiveResponse,
		Bolt11ReceiveForHashRequest, Bolt11ReceiveForHashResponse,
		Bolt11ClaimForHashRequest, Bolt11ClaimForHashResponse,
		Bolt11FailForHashRequest, Bolt11FailForHashResponse,
		Bolt11ReceiveViaJitChannelRequest, Bolt11ReceiveViaJitChannelResponse,
		Bolt11ReceiveVariableAmountViaJitChannelRequest, Bolt11ReceiveVariableAmountViaJitChannelResponse,
		Bolt11SendRequest, Bolt11SendResponse,
		Bolt12ReceiveRequest, Bolt12ReceiveResponse,
		Bolt12SendRequest, Bolt12SendResponse,
		SpontaneousSendRequest, SpontaneousSendResponse,
		OpenChannelRequest, OpenChannelResponse,
		SpliceInRequest, SpliceInResponse,
		SpliceOutRequest, SpliceOutResponse,
		CloseChannelRequest, CloseChannelResponse,
		ForceCloseChannelRequest, ForceCloseChannelResponse,
		ListChannelsRequest, ListChannelsResponse,
		UpdateChannelConfigRequest, UpdateChannelConfigResponse,
		GetPaymentDetailsRequest, GetPaymentDetailsResponse,
		ListPaymentsRequest, ListPaymentsResponse,
		ListForwardedPaymentsRequest, ListForwardedPaymentsResponse,
		ListPeersRequest, ListPeersResponse,
		ConnectPeerRequest, ConnectPeerResponse,
		DisconnectPeerRequest, DisconnectPeerResponse,
		SignMessageRequest, SignMessageResponse,
		VerifySignatureRequest, VerifySignatureResponse,
		ExportPathfindingScoresRequest, ExportPathfindingScoresResponse,
		UnifiedSendRequest, UnifiedSendResponse, UnifiedSendPaymentResult,
		GraphListChannelsRequest, GraphListChannelsResponse,
		GraphGetChannelRequest, GraphGetChannelResponse,
		GraphListNodesRequest, GraphListNodesResponse,
		GraphGetNodeRequest, GraphGetNodeResponse,
		// Domain types
		Payment, PaymentKind, PaymentDirection, PaymentStatus,
		Onchain, ConfirmationStatus, Confirmed, Unconfirmed,
		Bolt11, Bolt11Jit, Bolt12Offer, Bolt12Refund, Spontaneous,
		LspFeeLimits, ForwardedPayment,
		Channel, ChannelConfig, MaxDustHtlcExposure, OutPoint, BestBlock,
		LightningBalance, ClaimableOnChannelClose, ClaimableAwaitingConfirmations,
		ContentiousClaimable, MaybeTimeoutClaimableHtlc, MaybePreimageClaimableHtlc,
		CounterpartyRevokedOutputClaimable,
		PendingSweepBalance, PendingBroadcast, BroadcastAwaitingConfirmation,
		AwaitingThresholdConfirmations,
		PageToken, Bolt11InvoiceDescription, RouteParametersConfig, BalanceSource,
		GraphRoutingFees, GraphChannelUpdate, GraphChannel,
		GraphNodeAnnouncement, GraphNode, Peer,
		// Event types
		Event, PaymentReceived, PaymentSuccessful, PaymentFailed,
		PaymentClaimable, PaymentForwarded,
		// Error types
		ErrorResponse, ErrorCode,
	)),
	modifiers(&HmacSecurityAddon),
	tags(
		(name = "Node", description = "Node information and balances"),
		(name = "Onchain", description = "On-chain wallet operations"),
		(name = "Bolt11", description = "BOLT11 Lightning invoice operations"),
		(name = "Bolt12", description = "BOLT12 Lightning offer operations"),
		(name = "Channels", description = "Channel management"),
		(name = "Payments", description = "Payment queries"),
		(name = "Peers", description = "Peer management"),
		(name = "Send", description = "Sending payments"),
		(name = "Graph", description = "Network graph queries"),
		(name = "Crypto", description = "Message signing and verification"),
		(name = "Events", description = "Server-Sent Events for payment notifications"),
	)
)]
pub(crate) struct ApiDoc;

struct HmacSecurityAddon;

impl Modify for HmacSecurityAddon {
	fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
		if let Some(components) = openapi.components.as_mut() {
			components.add_security_scheme(
				"hmac_auth",
				SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-Auth"))),
			);
		}
	}
}

#[utoipa::path(
	post, path = "/GetNodeInfo",
	request_body = GetNodeInfoRequest,
	responses(
		(status = 200, body = GetNodeInfoResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Node"
)]
fn get_node_info() {}

#[utoipa::path(
	post, path = "/GetBalances",
	request_body = GetBalancesRequest,
	responses(
		(status = 200, body = GetBalancesResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Node"
)]
fn get_balances() {}

#[utoipa::path(
	post, path = "/OnchainReceive",
	request_body = OnchainReceiveRequest,
	responses(
		(status = 200, body = OnchainReceiveResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Onchain"
)]
fn onchain_receive() {}

#[utoipa::path(
	post, path = "/OnchainSend",
	request_body = OnchainSendRequest,
	responses(
		(status = 200, body = OnchainSendResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Onchain"
)]
fn onchain_send() {}

#[utoipa::path(
	post, path = "/Bolt11Receive",
	request_body = Bolt11ReceiveRequest,
	responses(
		(status = 200, body = Bolt11ReceiveResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Bolt11"
)]
fn bolt11_receive() {}

#[utoipa::path(
	post, path = "/Bolt11ReceiveForHash",
	request_body = Bolt11ReceiveForHashRequest,
	responses(
		(status = 200, body = Bolt11ReceiveForHashResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Bolt11"
)]
fn bolt11_receive_for_hash() {}

#[utoipa::path(
	post, path = "/Bolt11ClaimForHash",
	request_body = Bolt11ClaimForHashRequest,
	responses(
		(status = 200, body = Bolt11ClaimForHashResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Bolt11"
)]
fn bolt11_claim_for_hash() {}

#[utoipa::path(
	post, path = "/Bolt11FailForHash",
	request_body = Bolt11FailForHashRequest,
	responses(
		(status = 200, body = Bolt11FailForHashResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Bolt11"
)]
fn bolt11_fail_for_hash() {}

#[utoipa::path(
	post, path = "/Bolt11ReceiveViaJitChannel",
	request_body = Bolt11ReceiveViaJitChannelRequest,
	responses(
		(status = 200, body = Bolt11ReceiveViaJitChannelResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Bolt11"
)]
fn bolt11_receive_via_jit_channel() {}

#[utoipa::path(
	post, path = "/Bolt11ReceiveVariableAmountViaJitChannel",
	request_body = Bolt11ReceiveVariableAmountViaJitChannelRequest,
	responses(
		(status = 200, body = Bolt11ReceiveVariableAmountViaJitChannelResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Bolt11"
)]
fn bolt11_receive_variable_amount_via_jit_channel() {}

#[utoipa::path(
	post, path = "/Bolt11Send",
	request_body = Bolt11SendRequest,
	responses(
		(status = 200, body = Bolt11SendResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Bolt11"
)]
fn bolt11_send() {}

#[utoipa::path(
	post, path = "/Bolt12Receive",
	request_body = Bolt12ReceiveRequest,
	responses(
		(status = 200, body = Bolt12ReceiveResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Bolt12"
)]
fn bolt12_receive() {}

#[utoipa::path(
	post, path = "/Bolt12Send",
	request_body = Bolt12SendRequest,
	responses(
		(status = 200, body = Bolt12SendResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Bolt12"
)]
fn bolt12_send() {}

#[utoipa::path(
	post, path = "/SpontaneousSend",
	request_body = SpontaneousSendRequest,
	responses(
		(status = 200, body = SpontaneousSendResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Send"
)]
fn spontaneous_send() {}

#[utoipa::path(
	post, path = "/OpenChannel",
	request_body = OpenChannelRequest,
	responses(
		(status = 200, body = OpenChannelResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Channels"
)]
fn open_channel() {}

#[utoipa::path(
	post, path = "/SpliceIn",
	request_body = SpliceInRequest,
	responses(
		(status = 200, body = SpliceInResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Channels"
)]
fn splice_in() {}

#[utoipa::path(
	post, path = "/SpliceOut",
	request_body = SpliceOutRequest,
	responses(
		(status = 200, body = SpliceOutResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Channels"
)]
fn splice_out() {}

#[utoipa::path(
	post, path = "/CloseChannel",
	request_body = CloseChannelRequest,
	responses(
		(status = 200, body = CloseChannelResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Channels"
)]
fn close_channel() {}

#[utoipa::path(
	post, path = "/ForceCloseChannel",
	request_body = ForceCloseChannelRequest,
	responses(
		(status = 200, body = ForceCloseChannelResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Channels"
)]
fn force_close_channel() {}

#[utoipa::path(
	post, path = "/ListChannels",
	request_body = ListChannelsRequest,
	responses(
		(status = 200, body = ListChannelsResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Channels"
)]
fn list_channels() {}

#[utoipa::path(
	post, path = "/UpdateChannelConfig",
	request_body = UpdateChannelConfigRequest,
	responses(
		(status = 200, body = UpdateChannelConfigResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Channels"
)]
fn update_channel_config() {}

#[utoipa::path(
	post, path = "/GetPaymentDetails",
	request_body = GetPaymentDetailsRequest,
	responses(
		(status = 200, body = GetPaymentDetailsResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Payments"
)]
fn get_payment_details() {}

#[utoipa::path(
	post, path = "/ListPayments",
	request_body = ListPaymentsRequest,
	responses(
		(status = 200, body = ListPaymentsResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Payments"
)]
fn list_payments() {}

#[utoipa::path(
	post, path = "/ListForwardedPayments",
	request_body = ListForwardedPaymentsRequest,
	responses(
		(status = 200, body = ListForwardedPaymentsResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Payments"
)]
fn list_forwarded_payments() {}

#[utoipa::path(
	post, path = "/ListPeers",
	request_body = ListPeersRequest,
	responses(
		(status = 200, body = ListPeersResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Peers"
)]
fn list_peers() {}

#[utoipa::path(
	post, path = "/ConnectPeer",
	request_body = ConnectPeerRequest,
	responses(
		(status = 200, body = ConnectPeerResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Peers"
)]
fn connect_peer() {}

#[utoipa::path(
	post, path = "/DisconnectPeer",
	request_body = DisconnectPeerRequest,
	responses(
		(status = 200, body = DisconnectPeerResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Peers"
)]
fn disconnect_peer() {}

#[utoipa::path(
	post, path = "/SignMessage",
	request_body = SignMessageRequest,
	responses(
		(status = 200, body = SignMessageResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Crypto"
)]
fn sign_message() {}

#[utoipa::path(
	post, path = "/VerifySignature",
	request_body = VerifySignatureRequest,
	responses(
		(status = 200, body = VerifySignatureResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Crypto"
)]
fn verify_signature() {}

#[utoipa::path(
	post, path = "/ExportPathfindingScores",
	request_body = ExportPathfindingScoresRequest,
	responses(
		(status = 200, body = ExportPathfindingScoresResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Node"
)]
fn export_pathfinding_scores() {}

#[utoipa::path(
	post, path = "/UnifiedSend",
	request_body = UnifiedSendRequest,
	responses(
		(status = 200, body = UnifiedSendResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Send"
)]
fn unified_send() {}

#[utoipa::path(
	post, path = "/GraphListChannels",
	request_body = GraphListChannelsRequest,
	responses(
		(status = 200, body = GraphListChannelsResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Graph"
)]
fn graph_list_channels() {}

#[utoipa::path(
	post, path = "/GraphGetChannel",
	request_body = GraphGetChannelRequest,
	responses(
		(status = 200, body = GraphGetChannelResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Graph"
)]
fn graph_get_channel() {}

#[utoipa::path(
	post, path = "/GraphListNodes",
	request_body = GraphListNodesRequest,
	responses(
		(status = 200, body = GraphListNodesResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Graph"
)]
fn graph_list_nodes() {}

#[utoipa::path(
	post, path = "/GraphGetNode",
	request_body = GraphGetNodeRequest,
	responses(
		(status = 200, body = GraphGetNodeResponse),
		(status = 400, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Graph"
)]
fn graph_get_node() {}

#[utoipa::path(
	post, path = "/Subscribe",
	responses(
		(status = 200, content_type = "text/event-stream", description = "Server-Sent Events stream of payment lifecycle events"),
		(status = 401, body = ErrorResponse),
	),
	security(("hmac_auth" = [])),
	tag = "Events"
)]
fn subscribe() {}
