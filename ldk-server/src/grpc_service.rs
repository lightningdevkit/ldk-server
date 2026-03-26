// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::pin::Pin;
use std::sync::Arc;

use ldk_node::bitcoin::hashes::hmac::{Hmac, HmacEngine};
use ldk_node::bitcoin::hashes::{sha256, Hash, HashEngine};
use ldk_node::Node;
use ldk_server_protos::api::lightning_node_server::LightningNode;
use ldk_server_protos::api::*;
use ldk_server_protos::events::EventEnvelope;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tonic::service::Interceptor;
use tonic::{Request, Response, Status};

use crate::api::bolt11_claim_for_hash::handle_bolt11_claim_for_hash_request;
use crate::api::bolt11_fail_for_hash::handle_bolt11_fail_for_hash_request;
use crate::api::bolt11_receive::handle_bolt11_receive_request;
use crate::api::bolt11_receive_for_hash::handle_bolt11_receive_for_hash_request;
use crate::api::bolt11_receive_via_jit_channel::{
	handle_bolt11_receive_variable_amount_via_jit_channel_request,
	handle_bolt11_receive_via_jit_channel_request,
};
use crate::api::bolt11_send::handle_bolt11_send_request;
use crate::api::bolt12_receive::handle_bolt12_receive_request;
use crate::api::bolt12_send::handle_bolt12_send_request;
use crate::api::close_channel::{handle_close_channel_request, handle_force_close_channel_request};
use crate::api::connect_peer::handle_connect_peer;
use crate::api::decode_invoice::handle_decode_invoice_request;
use crate::api::decode_offer::handle_decode_offer_request;
use crate::api::disconnect_peer::handle_disconnect_peer;
use crate::api::error::{LdkServerError, LdkServerErrorCode};
use crate::api::export_pathfinding_scores::handle_export_pathfinding_scores_request;
use crate::api::get_balances::handle_get_balances_request;
use crate::api::get_node_info::handle_get_node_info_request;
use crate::api::get_payment_details::handle_get_payment_details_request;
use crate::api::graph_get_channel::handle_graph_get_channel_request;
use crate::api::graph_get_node::handle_graph_get_node_request;
use crate::api::graph_list_channels::handle_graph_list_channels_request;
use crate::api::graph_list_nodes::handle_graph_list_nodes_request;
use crate::api::list_channels::handle_list_channels_request;
use crate::api::list_forwarded_payments::handle_list_forwarded_payments_request;
use crate::api::list_payments::handle_list_payments_request;
use crate::api::list_peers::handle_list_peers_request;
use crate::api::onchain_receive::handle_onchain_receive_request;
use crate::api::onchain_send::handle_onchain_send_request;
use crate::api::open_channel::handle_open_channel;
use crate::api::sign_message::handle_sign_message_request;
use crate::api::splice_channel::{handle_splice_in_request, handle_splice_out_request};
use crate::api::spontaneous_send::handle_spontaneous_send_request;
use crate::api::unified_send::handle_unified_send_request;
use crate::api::update_channel_config::handle_update_channel_config_request;
use crate::api::verify_signature::handle_verify_signature_request;
use crate::io::persist::paginated_kv_store::PaginatedKVStore;

/// Maximum allowed time difference between client timestamp and server time (1 minute).
const AUTH_TIMESTAMP_TOLERANCE_SECS: u64 = 60;

/// Interceptor that validates HMAC auth metadata on incoming gRPC requests.
#[derive(Clone)]
pub(crate) struct AuthInterceptor {
	api_key: String,
}

impl AuthInterceptor {
	pub(crate) fn new(api_key: String) -> Self {
		Self { api_key }
	}
}

impl Interceptor for AuthInterceptor {
	fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status> {
		let auth_header = request
			.metadata()
			.get("x-auth")
			.and_then(|v| v.to_str().ok())
			.ok_or_else(|| Status::unauthenticated("Missing x-auth metadata"))?;

		let auth_data = auth_header
			.strip_prefix("HMAC ")
			.ok_or_else(|| Status::unauthenticated("Invalid x-auth format"))?;

		let (timestamp_str, provided_hmac_hex) = auth_data
			.split_once(':')
			.ok_or_else(|| Status::unauthenticated("Invalid x-auth format"))?;

		let timestamp = timestamp_str
			.parse::<u64>()
			.map_err(|_| Status::unauthenticated("Invalid timestamp in x-auth"))?;

		let now = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map_err(|_| Status::internal("System time error"))?
			.as_secs();

		if now.abs_diff(timestamp) > AUTH_TIMESTAMP_TOLERANCE_SECS {
			return Err(Status::unauthenticated("Request timestamp expired"));
		}

		let mut hmac_engine: HmacEngine<sha256::Hash> = HmacEngine::new(self.api_key.as_bytes());
		hmac_engine.input(&timestamp.to_be_bytes());
		let expected_hmac = Hmac::<sha256::Hash>::from_engine(hmac_engine);

		// Parse provided HMAC and compare using the constant-time Hash equality
		let provided_hmac = provided_hmac_hex
			.parse::<Hmac<sha256::Hash>>()
			.map_err(|_| Status::unauthenticated("Invalid HMAC in x-auth"))?;

		if expected_hmac != provided_hmac {
			return Err(Status::unauthenticated("Invalid credentials"));
		}

		Ok(request)
	}
}

pub(crate) struct Context {
	pub(crate) node: Arc<Node>,
	pub(crate) paginated_kv_store: Arc<dyn PaginatedKVStore>,
}

pub(crate) struct NodeGrpcService {
	context: Context,
	event_sender: broadcast::Sender<EventEnvelope>,
}

impl NodeGrpcService {
	pub(crate) fn new(
		node: Arc<Node>, paginated_kv_store: Arc<dyn PaginatedKVStore>,
		event_sender: broadcast::Sender<EventEnvelope>,
	) -> Self {
		Self { context: Context { node, paginated_kv_store }, event_sender }
	}
}

fn into_status(e: LdkServerError) -> Status {
	let code = match e.error_code {
		LdkServerErrorCode::InvalidRequestError => tonic::Code::InvalidArgument,
		LdkServerErrorCode::AuthError => tonic::Code::Unauthenticated,
		LdkServerErrorCode::LightningError => tonic::Code::FailedPrecondition,
		LdkServerErrorCode::InternalServerError => tonic::Code::Internal,
	};
	Status::new(code, e.message)
}

/// A helper macro to implement a unary gRPC method by delegating to an existing handler function.
macro_rules! impl_unary {
	($self:ident, $request:ident, $handler:ident) => {{
		$handler(&$self.context, $request.into_inner()).map(Response::new).map_err(into_status)
	}};
}

type EventStream = Pin<Box<dyn tokio_stream::Stream<Item = Result<EventEnvelope, Status>> + Send>>;

#[tonic::async_trait]
impl LightningNode for NodeGrpcService {
	async fn get_node_info(
		&self, request: Request<GetNodeInfoRequest>,
	) -> Result<Response<GetNodeInfoResponse>, Status> {
		impl_unary!(self, request, handle_get_node_info_request)
	}

	async fn get_balances(
		&self, request: Request<GetBalancesRequest>,
	) -> Result<Response<GetBalancesResponse>, Status> {
		impl_unary!(self, request, handle_get_balances_request)
	}

	async fn onchain_receive(
		&self, request: Request<OnchainReceiveRequest>,
	) -> Result<Response<OnchainReceiveResponse>, Status> {
		impl_unary!(self, request, handle_onchain_receive_request)
	}

	async fn onchain_send(
		&self, request: Request<OnchainSendRequest>,
	) -> Result<Response<OnchainSendResponse>, Status> {
		impl_unary!(self, request, handle_onchain_send_request)
	}

	async fn bolt11_receive(
		&self, request: Request<Bolt11ReceiveRequest>,
	) -> Result<Response<Bolt11ReceiveResponse>, Status> {
		impl_unary!(self, request, handle_bolt11_receive_request)
	}

	async fn bolt11_receive_for_hash(
		&self, request: Request<Bolt11ReceiveForHashRequest>,
	) -> Result<Response<Bolt11ReceiveForHashResponse>, Status> {
		impl_unary!(self, request, handle_bolt11_receive_for_hash_request)
	}

	async fn bolt11_claim_for_hash(
		&self, request: Request<Bolt11ClaimForHashRequest>,
	) -> Result<Response<Bolt11ClaimForHashResponse>, Status> {
		impl_unary!(self, request, handle_bolt11_claim_for_hash_request)
	}

	async fn bolt11_fail_for_hash(
		&self, request: Request<Bolt11FailForHashRequest>,
	) -> Result<Response<Bolt11FailForHashResponse>, Status> {
		impl_unary!(self, request, handle_bolt11_fail_for_hash_request)
	}

	async fn bolt11_receive_via_jit_channel(
		&self, request: Request<Bolt11ReceiveViaJitChannelRequest>,
	) -> Result<Response<Bolt11ReceiveViaJitChannelResponse>, Status> {
		impl_unary!(self, request, handle_bolt11_receive_via_jit_channel_request)
	}

	async fn bolt11_receive_variable_amount_via_jit_channel(
		&self, request: Request<Bolt11ReceiveVariableAmountViaJitChannelRequest>,
	) -> Result<Response<Bolt11ReceiveVariableAmountViaJitChannelResponse>, Status> {
		impl_unary!(self, request, handle_bolt11_receive_variable_amount_via_jit_channel_request)
	}

	async fn bolt11_send(
		&self, request: Request<Bolt11SendRequest>,
	) -> Result<Response<Bolt11SendResponse>, Status> {
		impl_unary!(self, request, handle_bolt11_send_request)
	}

	async fn bolt12_receive(
		&self, request: Request<Bolt12ReceiveRequest>,
	) -> Result<Response<Bolt12ReceiveResponse>, Status> {
		impl_unary!(self, request, handle_bolt12_receive_request)
	}

	async fn bolt12_send(
		&self, request: Request<Bolt12SendRequest>,
	) -> Result<Response<Bolt12SendResponse>, Status> {
		impl_unary!(self, request, handle_bolt12_send_request)
	}

	async fn spontaneous_send(
		&self, request: Request<SpontaneousSendRequest>,
	) -> Result<Response<SpontaneousSendResponse>, Status> {
		impl_unary!(self, request, handle_spontaneous_send_request)
	}

	async fn open_channel(
		&self, request: Request<OpenChannelRequest>,
	) -> Result<Response<OpenChannelResponse>, Status> {
		impl_unary!(self, request, handle_open_channel)
	}

	async fn splice_in(
		&self, request: Request<SpliceInRequest>,
	) -> Result<Response<SpliceInResponse>, Status> {
		impl_unary!(self, request, handle_splice_in_request)
	}

	async fn splice_out(
		&self, request: Request<SpliceOutRequest>,
	) -> Result<Response<SpliceOutResponse>, Status> {
		impl_unary!(self, request, handle_splice_out_request)
	}

	async fn update_channel_config(
		&self, request: Request<UpdateChannelConfigRequest>,
	) -> Result<Response<UpdateChannelConfigResponse>, Status> {
		impl_unary!(self, request, handle_update_channel_config_request)
	}

	async fn close_channel(
		&self, request: Request<CloseChannelRequest>,
	) -> Result<Response<CloseChannelResponse>, Status> {
		impl_unary!(self, request, handle_close_channel_request)
	}

	async fn force_close_channel(
		&self, request: Request<ForceCloseChannelRequest>,
	) -> Result<Response<ForceCloseChannelResponse>, Status> {
		impl_unary!(self, request, handle_force_close_channel_request)
	}

	async fn list_channels(
		&self, request: Request<ListChannelsRequest>,
	) -> Result<Response<ListChannelsResponse>, Status> {
		impl_unary!(self, request, handle_list_channels_request)
	}

	async fn get_payment_details(
		&self, request: Request<GetPaymentDetailsRequest>,
	) -> Result<Response<GetPaymentDetailsResponse>, Status> {
		impl_unary!(self, request, handle_get_payment_details_request)
	}

	async fn list_payments(
		&self, request: Request<ListPaymentsRequest>,
	) -> Result<Response<ListPaymentsResponse>, Status> {
		impl_unary!(self, request, handle_list_payments_request)
	}

	async fn list_forwarded_payments(
		&self, request: Request<ListForwardedPaymentsRequest>,
	) -> Result<Response<ListForwardedPaymentsResponse>, Status> {
		impl_unary!(self, request, handle_list_forwarded_payments_request)
	}

	async fn connect_peer(
		&self, request: Request<ConnectPeerRequest>,
	) -> Result<Response<ConnectPeerResponse>, Status> {
		impl_unary!(self, request, handle_connect_peer)
	}

	async fn disconnect_peer(
		&self, request: Request<DisconnectPeerRequest>,
	) -> Result<Response<DisconnectPeerResponse>, Status> {
		impl_unary!(self, request, handle_disconnect_peer)
	}

	async fn list_peers(
		&self, request: Request<ListPeersRequest>,
	) -> Result<Response<ListPeersResponse>, Status> {
		impl_unary!(self, request, handle_list_peers_request)
	}

	async fn sign_message(
		&self, request: Request<SignMessageRequest>,
	) -> Result<Response<SignMessageResponse>, Status> {
		impl_unary!(self, request, handle_sign_message_request)
	}

	async fn verify_signature(
		&self, request: Request<VerifySignatureRequest>,
	) -> Result<Response<VerifySignatureResponse>, Status> {
		impl_unary!(self, request, handle_verify_signature_request)
	}

	async fn export_pathfinding_scores(
		&self, request: Request<ExportPathfindingScoresRequest>,
	) -> Result<Response<ExportPathfindingScoresResponse>, Status> {
		impl_unary!(self, request, handle_export_pathfinding_scores_request)
	}

	async fn unified_send(
		&self, request: Request<UnifiedSendRequest>,
	) -> Result<Response<UnifiedSendResponse>, Status> {
		impl_unary!(self, request, handle_unified_send_request)
	}

	async fn decode_invoice(
		&self, request: Request<DecodeInvoiceRequest>,
	) -> Result<Response<DecodeInvoiceResponse>, Status> {
		impl_unary!(self, request, handle_decode_invoice_request)
	}

	async fn decode_offer(
		&self, request: Request<DecodeOfferRequest>,
	) -> Result<Response<DecodeOfferResponse>, Status> {
		impl_unary!(self, request, handle_decode_offer_request)
	}

	async fn graph_list_channels(
		&self, request: Request<GraphListChannelsRequest>,
	) -> Result<Response<GraphListChannelsResponse>, Status> {
		impl_unary!(self, request, handle_graph_list_channels_request)
	}

	async fn graph_get_channel(
		&self, request: Request<GraphGetChannelRequest>,
	) -> Result<Response<GraphGetChannelResponse>, Status> {
		impl_unary!(self, request, handle_graph_get_channel_request)
	}

	async fn graph_list_nodes(
		&self, request: Request<GraphListNodesRequest>,
	) -> Result<Response<GraphListNodesResponse>, Status> {
		impl_unary!(self, request, handle_graph_list_nodes_request)
	}

	async fn graph_get_node(
		&self, request: Request<GraphGetNodeRequest>,
	) -> Result<Response<GraphGetNodeResponse>, Status> {
		impl_unary!(self, request, handle_graph_get_node_request)
	}

	type SubscribeEventsStream = EventStream;

	async fn subscribe_events(
		&self, _request: Request<SubscribeEventsRequest>,
	) -> Result<Response<Self::SubscribeEventsStream>, Status> {
		let rx = self.event_sender.subscribe();
		let stream = BroadcastStream::new(rx).filter_map(|result| match result {
			Ok(event) => Some(Ok(event)),
			Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
				log::warn!("Event subscriber lagged by {} events", n);
				None
			},
		});
		Ok(Response::new(Box::pin(stream)))
	}
}
