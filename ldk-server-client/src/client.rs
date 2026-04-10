// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use bitcoin_hashes::hmac::{Hmac, HmacEngine};
use bitcoin_hashes::{sha256, Hash, HashEngine};
use hyper::body::HttpBody as _;
use hyper::{Body as HyperBody, Client as HyperClient, Request as HyperRequest, Version};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use ldk_server_grpc::api::SubscribeEventsRequest;
use ldk_server_grpc::api::{
	Bolt11ClaimForHashRequest, Bolt11ClaimForHashResponse, Bolt11FailForHashRequest,
	Bolt11FailForHashResponse, Bolt11ReceiveForHashRequest, Bolt11ReceiveForHashResponse,
	Bolt11ReceiveRequest, Bolt11ReceiveResponse, Bolt11ReceiveVariableAmountViaJitChannelRequest,
	Bolt11ReceiveVariableAmountViaJitChannelResponse, Bolt11ReceiveViaJitChannelRequest,
	Bolt11ReceiveViaJitChannelResponse, Bolt11SendRequest, Bolt11SendResponse,
	Bolt12ReceiveRequest, Bolt12ReceiveResponse, Bolt12SendRequest, Bolt12SendResponse,
	CloseChannelRequest, CloseChannelResponse, ConnectPeerRequest, ConnectPeerResponse,
	DecodeInvoiceRequest, DecodeInvoiceResponse, DecodeOfferRequest, DecodeOfferResponse,
	DisconnectPeerRequest, DisconnectPeerResponse, ExportPathfindingScoresRequest,
	ExportPathfindingScoresResponse, ForceCloseChannelRequest, ForceCloseChannelResponse,
	GetBalancesRequest, GetBalancesResponse, GetNodeInfoRequest, GetNodeInfoResponse,
	GetPaymentDetailsRequest, GetPaymentDetailsResponse, GraphGetChannelRequest,
	GraphGetChannelResponse, GraphGetNodeRequest, GraphGetNodeResponse, GraphListChannelsRequest,
	GraphListChannelsResponse, GraphListNodesRequest, GraphListNodesResponse, ListChannelsRequest,
	ListChannelsResponse, ListForwardedPaymentsRequest, ListForwardedPaymentsResponse,
	ListPaymentsRequest, ListPaymentsResponse, ListPeersRequest, ListPeersResponse,
	OnchainReceiveRequest, OnchainReceiveResponse, OnchainSendRequest, OnchainSendResponse,
	OpenChannelRequest, OpenChannelResponse, SignMessageRequest, SignMessageResponse,
	SpliceInRequest, SpliceInResponse, SpliceOutRequest, SpliceOutResponse, SpontaneousSendRequest,
	SpontaneousSendResponse, UnifiedSendRequest, UnifiedSendResponse, UpdateChannelConfigRequest,
	UpdateChannelConfigResponse, VerifySignatureRequest, VerifySignatureResponse,
};
use ldk_server_grpc::endpoints::{
	BOLT11_CLAIM_FOR_HASH_PATH, BOLT11_FAIL_FOR_HASH_PATH, BOLT11_RECEIVE_FOR_HASH_PATH,
	BOLT11_RECEIVE_PATH, BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH,
	BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH, BOLT11_SEND_PATH, BOLT12_RECEIVE_PATH, BOLT12_SEND_PATH,
	CLOSE_CHANNEL_PATH, CONNECT_PEER_PATH, DECODE_INVOICE_PATH, DECODE_OFFER_PATH,
	DISCONNECT_PEER_PATH, EXPORT_PATHFINDING_SCORES_PATH, FORCE_CLOSE_CHANNEL_PATH,
	GET_BALANCES_PATH, GET_METRICS_PATH, GET_NODE_INFO_PATH, GET_PAYMENT_DETAILS_PATH,
	GRAPH_GET_CHANNEL_PATH, GRAPH_GET_NODE_PATH, GRAPH_LIST_CHANNELS_PATH, GRAPH_LIST_NODES_PATH,
	GRPC_SERVICE_PREFIX, LIST_CHANNELS_PATH, LIST_FORWARDED_PAYMENTS_PATH, LIST_PAYMENTS_PATH,
	LIST_PEERS_PATH, ONCHAIN_RECEIVE_PATH, ONCHAIN_SEND_PATH, OPEN_CHANNEL_PATH, SIGN_MESSAGE_PATH,
	SPLICE_IN_PATH, SPLICE_OUT_PATH, SPONTANEOUS_SEND_PATH, SUBSCRIBE_EVENTS_PATH,
	UNIFIED_SEND_PATH, UPDATE_CHANNEL_CONFIG_PATH, VERIFY_SIGNATURE_PATH,
};
use ldk_server_grpc::events::EventEnvelope;
use ldk_server_grpc::grpc::{
	decode_grpc_body, encode_grpc_frame, percent_decode, GRPC_STATUS_FAILED_PRECONDITION,
	GRPC_STATUS_INTERNAL, GRPC_STATUS_INVALID_ARGUMENT, GRPC_STATUS_OK,
	GRPC_STATUS_UNAUTHENTICATED, GRPC_STATUS_UNAVAILABLE,
};
use prost::Message;
use reqwest::{header::HeaderMap, Certificate, Client};
use rustls::{ClientConfig, RootCertStore};
use rustls_pemfile::certs;

use crate::error::LdkServerError;
use crate::error::LdkServerErrorCode::{
	AuthError, InternalError, InternalServerError, InvalidRequestError, LightningError,
};

type StreamingClient = HyperClient<HttpsConnector<hyper::client::HttpConnector>, HyperBody>;

/// Client to access a hosted instance of LDK Server via gRPC.
///
/// The client requires the server's TLS certificate to be provided for verification.
/// This certificate can be found at `<server_storage_dir>/tls.crt` after the
/// server generates it on first startup.
#[derive(Clone)]
pub struct LdkServerClient {
	base_url: String,
	client: Client,
	streaming_client: StreamingClient,
	api_key: String,
}

impl LdkServerClient {
	/// Constructs a [`LdkServerClient`] using `base_url` as the ldk-server endpoint.
	///
	/// `base_url` should not include the scheme, e.g., `localhost:3000`.
	/// `api_key` is used for HMAC-based authentication.
	/// `server_cert_pem` is the server's TLS certificate in PEM format. This can be
	/// found at `<server_storage_dir>/tls.crt` after the server starts.
	pub fn new(base_url: String, api_key: String, server_cert_pem: &[u8]) -> Result<Self, String> {
		let cert = Certificate::from_pem(server_cert_pem)
			.map_err(|e| format!("Failed to parse server certificate: {e}"))?;
		let streaming_client = build_streaming_client(server_cert_pem)?;

		let client = Client::builder()
			.add_root_certificate(cert)
			.build()
			.map_err(|e| format!("Failed to build HTTP client: {e}"))?;

		Ok(Self { base_url, client, streaming_client, api_key })
	}

	/// Computes the HMAC-SHA256 authentication header value.
	/// Format: "HMAC <timestamp>:<hmac_hex>"
	/// Uses timestamp-only HMAC (no body) since TLS guarantees integrity.
	fn compute_auth_header(&self) -> String {
		let timestamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.expect("System time should be after Unix epoch")
			.as_secs();

		// HMAC-SHA256(api_key, timestamp_bytes) — no body
		let mut hmac_engine: HmacEngine<sha256::Hash> = HmacEngine::new(self.api_key.as_bytes());
		hmac_engine.input(&timestamp.to_be_bytes());
		let hmac_result = Hmac::<sha256::Hash>::from_engine(hmac_engine);

		format!("HMAC {}:{}", timestamp, hmac_result)
	}

	/// Retrieve the latest node info like `node_id`, `current_best_block` etc.
	pub async fn get_node_info(
		&self, request: GetNodeInfoRequest,
	) -> Result<GetNodeInfoResponse, LdkServerError> {
		self.grpc_unary(&request, GET_NODE_INFO_PATH).await
	}

	/// Retrieve the node metrics in Prometheus format.
	pub async fn get_metrics(&self) -> Result<String, LdkServerError> {
		self.get_metrics_with_auth(None, None).await
	}

	/// Retrieve the node metrics in Prometheus format using Basic Auth.
	pub async fn get_metrics_with_auth(
		&self, username: Option<&str>, password: Option<&str>,
	) -> Result<String, LdkServerError> {
		let url = format!("https://{}/{GET_METRICS_PATH}", self.base_url);
		let mut builder = self.client.get(&url);
		if let (Some(u), Some(p)) = (username, password) {
			builder = builder.basic_auth(u, Some(p));
		}
		let response = builder.send().await.map_err(|e| {
			LdkServerError::new(InternalError, format!("HTTP request failed: {}", e))
		})?;
		if !response.status().is_success() {
			return Err(LdkServerError::new(
				InternalError,
				format!("Metrics request failed with status {}", response.status()),
			));
		}
		let payload = response.bytes().await.map_err(|e| {
			LdkServerError::new(InternalError, format!("Failed to read response body: {}", e))
		})?;
		String::from_utf8(payload.to_vec()).map_err(|e| {
			LdkServerError::new(
				InternalError,
				format!("Failed to decode metrics response as string: {}", e),
			)
		})
	}

	/// Retrieves an overview of all known balances.
	pub async fn get_balances(
		&self, request: GetBalancesRequest,
	) -> Result<GetBalancesResponse, LdkServerError> {
		self.grpc_unary(&request, GET_BALANCES_PATH).await
	}

	/// Retrieve a new on-chain funding address.
	pub async fn onchain_receive(
		&self, request: OnchainReceiveRequest,
	) -> Result<OnchainReceiveResponse, LdkServerError> {
		self.grpc_unary(&request, ONCHAIN_RECEIVE_PATH).await
	}

	/// Send an on-chain payment to the given address.
	pub async fn onchain_send(
		&self, request: OnchainSendRequest,
	) -> Result<OnchainSendResponse, LdkServerError> {
		self.grpc_unary(&request, ONCHAIN_SEND_PATH).await
	}

	/// Retrieve a new BOLT11 payable invoice.
	pub async fn bolt11_receive(
		&self, request: Bolt11ReceiveRequest,
	) -> Result<Bolt11ReceiveResponse, LdkServerError> {
		self.grpc_unary(&request, BOLT11_RECEIVE_PATH).await
	}

	/// Retrieve a new BOLT11 payable invoice for a given payment hash.
	pub async fn bolt11_receive_for_hash(
		&self, request: Bolt11ReceiveForHashRequest,
	) -> Result<Bolt11ReceiveForHashResponse, LdkServerError> {
		self.grpc_unary(&request, BOLT11_RECEIVE_FOR_HASH_PATH).await
	}

	/// Manually claim a payment for a given payment hash.
	pub async fn bolt11_claim_for_hash(
		&self, request: Bolt11ClaimForHashRequest,
	) -> Result<Bolt11ClaimForHashResponse, LdkServerError> {
		self.grpc_unary(&request, BOLT11_CLAIM_FOR_HASH_PATH).await
	}

	/// Manually fail a payment for a given payment hash.
	pub async fn bolt11_fail_for_hash(
		&self, request: Bolt11FailForHashRequest,
	) -> Result<Bolt11FailForHashResponse, LdkServerError> {
		self.grpc_unary(&request, BOLT11_FAIL_FOR_HASH_PATH).await
	}

	/// Retrieve a new fixed-amount BOLT11 invoice for receiving via an LSPS2 JIT channel.
	pub async fn bolt11_receive_via_jit_channel(
		&self, request: Bolt11ReceiveViaJitChannelRequest,
	) -> Result<Bolt11ReceiveViaJitChannelResponse, LdkServerError> {
		self.grpc_unary(&request, BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH).await
	}

	/// Retrieve a new variable-amount BOLT11 invoice for receiving via an LSPS2 JIT channel.
	pub async fn bolt11_receive_variable_amount_via_jit_channel(
		&self, request: Bolt11ReceiveVariableAmountViaJitChannelRequest,
	) -> Result<Bolt11ReceiveVariableAmountViaJitChannelResponse, LdkServerError> {
		self.grpc_unary(&request, BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH).await
	}

	/// Send a payment for a BOLT11 invoice.
	pub async fn bolt11_send(
		&self, request: Bolt11SendRequest,
	) -> Result<Bolt11SendResponse, LdkServerError> {
		self.grpc_unary(&request, BOLT11_SEND_PATH).await
	}

	/// Retrieve a new BOLT12 offer.
	pub async fn bolt12_receive(
		&self, request: Bolt12ReceiveRequest,
	) -> Result<Bolt12ReceiveResponse, LdkServerError> {
		self.grpc_unary(&request, BOLT12_RECEIVE_PATH).await
	}

	/// Send a payment for a BOLT12 offer.
	pub async fn bolt12_send(
		&self, request: Bolt12SendRequest,
	) -> Result<Bolt12SendResponse, LdkServerError> {
		self.grpc_unary(&request, BOLT12_SEND_PATH).await
	}

	/// Creates a new outbound channel.
	pub async fn open_channel(
		&self, request: OpenChannelRequest,
	) -> Result<OpenChannelResponse, LdkServerError> {
		self.grpc_unary(&request, OPEN_CHANNEL_PATH).await
	}

	/// Splices funds into the channel specified by given request.
	pub async fn splice_in(
		&self, request: SpliceInRequest,
	) -> Result<SpliceInResponse, LdkServerError> {
		self.grpc_unary(&request, SPLICE_IN_PATH).await
	}

	/// Splices funds out of the channel specified by given request.
	pub async fn splice_out(
		&self, request: SpliceOutRequest,
	) -> Result<SpliceOutResponse, LdkServerError> {
		self.grpc_unary(&request, SPLICE_OUT_PATH).await
	}

	/// Closes the channel specified by given request.
	pub async fn close_channel(
		&self, request: CloseChannelRequest,
	) -> Result<CloseChannelResponse, LdkServerError> {
		self.grpc_unary(&request, CLOSE_CHANNEL_PATH).await
	}

	/// Force closes the channel specified by given request.
	pub async fn force_close_channel(
		&self, request: ForceCloseChannelRequest,
	) -> Result<ForceCloseChannelResponse, LdkServerError> {
		self.grpc_unary(&request, FORCE_CLOSE_CHANNEL_PATH).await
	}

	/// Retrieves list of known channels.
	pub async fn list_channels(
		&self, request: ListChannelsRequest,
	) -> Result<ListChannelsResponse, LdkServerError> {
		self.grpc_unary(&request, LIST_CHANNELS_PATH).await
	}

	/// Retrieves list of all payments sent or received by us.
	pub async fn list_payments(
		&self, request: ListPaymentsRequest,
	) -> Result<ListPaymentsResponse, LdkServerError> {
		self.grpc_unary(&request, LIST_PAYMENTS_PATH).await
	}

	/// Updates the config for a previously opened channel.
	pub async fn update_channel_config(
		&self, request: UpdateChannelConfigRequest,
	) -> Result<UpdateChannelConfigResponse, LdkServerError> {
		self.grpc_unary(&request, UPDATE_CHANNEL_CONFIG_PATH).await
	}

	/// Retrieves payment details for a given payment id.
	pub async fn get_payment_details(
		&self, request: GetPaymentDetailsRequest,
	) -> Result<GetPaymentDetailsResponse, LdkServerError> {
		self.grpc_unary(&request, GET_PAYMENT_DETAILS_PATH).await
	}

	/// Retrieves list of all forwarded payments.
	pub async fn list_forwarded_payments(
		&self, request: ListForwardedPaymentsRequest,
	) -> Result<ListForwardedPaymentsResponse, LdkServerError> {
		self.grpc_unary(&request, LIST_FORWARDED_PAYMENTS_PATH).await
	}

	/// Connect to a peer on the Lightning Network.
	pub async fn connect_peer(
		&self, request: ConnectPeerRequest,
	) -> Result<ConnectPeerResponse, LdkServerError> {
		self.grpc_unary(&request, CONNECT_PEER_PATH).await
	}

	/// Disconnect from a peer and remove it from the peer store.
	pub async fn disconnect_peer(
		&self, request: DisconnectPeerRequest,
	) -> Result<DisconnectPeerResponse, LdkServerError> {
		self.grpc_unary(&request, DISCONNECT_PEER_PATH).await
	}

	/// Retrieves list of peers.
	pub async fn list_peers(
		&self, request: ListPeersRequest,
	) -> Result<ListPeersResponse, LdkServerError> {
		self.grpc_unary(&request, LIST_PEERS_PATH).await
	}

	/// Send a spontaneous payment (keysend) to a node.
	pub async fn spontaneous_send(
		&self, request: SpontaneousSendRequest,
	) -> Result<SpontaneousSendResponse, LdkServerError> {
		self.grpc_unary(&request, SPONTANEOUS_SEND_PATH).await
	}

	/// Send a payment given a BIP 21 URI or BIP 353 Human-Readable Name.
	pub async fn unified_send(
		&self, request: UnifiedSendRequest,
	) -> Result<UnifiedSendResponse, LdkServerError> {
		self.grpc_unary(&request, UNIFIED_SEND_PATH).await
	}

	/// Decode a BOLT11 invoice and return its parsed fields.
	pub async fn decode_invoice(
		&self, request: DecodeInvoiceRequest,
	) -> Result<DecodeInvoiceResponse, LdkServerError> {
		self.grpc_unary(&request, DECODE_INVOICE_PATH).await
	}

	/// Decode a BOLT12 offer and return its parsed fields.
	pub async fn decode_offer(
		&self, request: DecodeOfferRequest,
	) -> Result<DecodeOfferResponse, LdkServerError> {
		self.grpc_unary(&request, DECODE_OFFER_PATH).await
	}

	/// Sign a message with the node's secret key.
	pub async fn sign_message(
		&self, request: SignMessageRequest,
	) -> Result<SignMessageResponse, LdkServerError> {
		self.grpc_unary(&request, SIGN_MESSAGE_PATH).await
	}

	/// Verify a signature against a message and public key.
	pub async fn verify_signature(
		&self, request: VerifySignatureRequest,
	) -> Result<VerifySignatureResponse, LdkServerError> {
		self.grpc_unary(&request, VERIFY_SIGNATURE_PATH).await
	}

	/// Export the pathfinding scores used by the router.
	pub async fn export_pathfinding_scores(
		&self, request: ExportPathfindingScoresRequest,
	) -> Result<ExportPathfindingScoresResponse, LdkServerError> {
		self.grpc_unary(&request, EXPORT_PATHFINDING_SCORES_PATH).await
	}

	/// Returns a list of all known short channel IDs in the network graph.
	pub async fn graph_list_channels(
		&self, request: GraphListChannelsRequest,
	) -> Result<GraphListChannelsResponse, LdkServerError> {
		self.grpc_unary(&request, GRAPH_LIST_CHANNELS_PATH).await
	}

	/// Returns information on a channel with the given short channel ID from the network graph.
	pub async fn graph_get_channel(
		&self, request: GraphGetChannelRequest,
	) -> Result<GraphGetChannelResponse, LdkServerError> {
		self.grpc_unary(&request, GRAPH_GET_CHANNEL_PATH).await
	}

	/// Returns a list of all known node IDs in the network graph.
	pub async fn graph_list_nodes(
		&self, request: GraphListNodesRequest,
	) -> Result<GraphListNodesResponse, LdkServerError> {
		self.grpc_unary(&request, GRAPH_LIST_NODES_PATH).await
	}

	/// Returns information on a node with the given ID from the network graph.
	pub async fn graph_get_node(
		&self, request: GraphGetNodeRequest,
	) -> Result<GraphGetNodeResponse, LdkServerError> {
		self.grpc_unary(&request, GRAPH_GET_NODE_PATH).await
	}

	/// Subscribe to a stream of server events via server-streaming gRPC.
	///
	/// Returns an [`EventStream`] that yields [`EventEnvelope`] messages as they arrive.
	pub async fn subscribe_events(&self) -> Result<EventStream, LdkServerError> {
		self.grpc_server_streaming(&SubscribeEventsRequest {}, SUBSCRIBE_EVENTS_PATH).await
	}

	/// Send a unary gRPC request and decode the response.
	async fn grpc_unary<Rq: Message, Rs: Message + Default>(
		&self, request: &Rq, method: &str,
	) -> Result<Rs, LdkServerError> {
		let grpc_body = encode_grpc_frame(&request.encode_to_vec()).to_vec();

		let url = format!("https://{}{}{}", self.base_url, GRPC_SERVICE_PREFIX, method);
		let auth_header = self.compute_auth_header();

		let response = self
			.client
			.post(&url)
			.header("content-type", "application/grpc+proto")
			.header("te", "trailers")
			.header("x-auth", auth_header)
			.body(grpc_body)
			.send()
			.await
			.map_err(|e| {
				LdkServerError::new(InternalError, format!("gRPC request failed: {}", e))
			})?;

		// Check for Trailers-Only error responses (grpc-status in response headers).
		// In gRPC, when there is no response body (error case), the server sends
		// grpc-status as part of the initial HEADERS frame, readable as a regular header.
		if let Some(error) = grpc_error_from_headers(response.headers()) {
			return Err(error);
		}

		// Read the response body
		let payload = response.bytes().await.map_err(|e| {
			LdkServerError::new(InternalError, format!("Failed to read response body: {}", e))
		})?;

		let proto_bytes = decode_grpc_body(&payload)
			.map_err(|e| LdkServerError::new(InternalError, e.message))?;

		Rs::decode(proto_bytes).map_err(|e| {
			LdkServerError::new(InternalError, format!("Failed to decode gRPC response: {}", e))
		})
	}

	/// Open a server-streaming gRPC call and return a [`GrpcStream`] that
	/// yields decoded messages of type `Rs` as they arrive.
	async fn grpc_server_streaming<Rq: Message, Rs: Message + Default>(
		&self, request: &Rq, method: &str,
	) -> Result<GrpcStream<Rs>, LdkServerError> {
		let grpc_body = encode_grpc_frame(&request.encode_to_vec()).to_vec();

		let url = format!("https://{}{}{}", self.base_url, GRPC_SERVICE_PREFIX, method);
		let auth_header = self.compute_auth_header();

		let response = self
			.streaming_client
			.request(
				HyperRequest::post(&url)
					.version(Version::HTTP_2)
					.header("content-type", "application/grpc+proto")
					.header("te", "trailers")
					.header("x-auth", auth_header)
					.body(HyperBody::from(grpc_body))
					.map_err(|e| {
						LdkServerError::new(
							InternalError,
							format!("Failed to build gRPC request: {e}"),
						)
					})?,
			)
			.await
			.map_err(|e| {
				LdkServerError::new(InternalError, format!("gRPC request failed: {}", e))
			})?;

		let (parts, body) = response.into_parts();
		if let Some(error) = grpc_error_from_headers(&parts.headers) {
			return Err(error);
		}

		Ok(GrpcStream {
			body,
			buf: Vec::new(),
			trailers_checked: false,
			_marker: std::marker::PhantomData,
		})
	}
}

/// Map a gRPC status code to an LdkServerError.
fn grpc_code_to_error(code: u32, message: String) -> LdkServerError {
	match code {
		GRPC_STATUS_INVALID_ARGUMENT => LdkServerError::new(InvalidRequestError, message),
		GRPC_STATUS_FAILED_PRECONDITION => LdkServerError::new(LightningError, message),
		GRPC_STATUS_INTERNAL => LdkServerError::new(InternalServerError, message),
		GRPC_STATUS_UNAVAILABLE => LdkServerError::new(
			InternalError,
			if message.is_empty() {
				"gRPC stream became unavailable".to_string()
			} else {
				format!("gRPC stream became unavailable: {message}")
			},
		),
		GRPC_STATUS_UNAUTHENTICATED => LdkServerError::new(AuthError, message),
		_ => LdkServerError::new(
			InternalError,
			if message.is_empty() {
				format!("gRPC status {code}")
			} else {
				format!("gRPC status {code}: {message}")
			},
		),
	}
}

fn grpc_error_from_headers(headers: &HeaderMap) -> Option<LdkServerError> {
	let code = headers.get("grpc-status")?.to_str().ok()?.parse::<u32>().ok()?;
	if code == GRPC_STATUS_OK {
		return None;
	}

	let message = headers
		.get("grpc-message")
		.and_then(|v| v.to_str().ok())
		.map(percent_decode)
		.unwrap_or_default();
	Some(grpc_code_to_error(code, message))
}

/// A server-streaming gRPC response that yields decoded protobuf messages of type `M`.
///
/// Call [`next_message`](GrpcStream::next_message) to receive the next message from the server.
pub struct GrpcStream<M: Message + Default> {
	body: hyper::Body,
	buf: Vec<u8>,
	trailers_checked: bool,
	_marker: std::marker::PhantomData<M>,
}

/// Type alias for a streaming response that yields [`EventEnvelope`] messages.
pub type EventStream = GrpcStream<EventEnvelope>;

impl<M: Message + Default> GrpcStream<M> {
	/// Wait for the next message from the server.
	///
	/// Returns `None` if the stream has ended.
	pub async fn next_message(&mut self) -> Option<Result<M, LdkServerError>> {
		loop {
			// Try to decode a complete gRPC frame from the buffer
			if self.buf.len() >= 5 {
				let msg_len =
					u32::from_be_bytes([self.buf[1], self.buf[2], self.buf[3], self.buf[4]])
						as usize;
				if self.buf.len() >= 5 + msg_len {
					let proto_bytes = &self.buf[5..5 + msg_len];
					let result = M::decode(proto_bytes).map_err(|e| {
						LdkServerError::new(
							InternalError,
							format!("Failed to decode gRPC stream message: {}", e),
						)
					});
					self.buf.drain(..5 + msg_len);
					return Some(result);
				}
			}

			// Need more data — read the next chunk from the response body
			match self.body.data().await {
				Some(Ok(chunk)) => self.buf.extend_from_slice(&chunk),
				Some(Err(e)) => {
					return Some(Err(LdkServerError::new(
						InternalError,
						format!("Failed to read gRPC stream: {}", e),
					)));
				},
				None => {
					if self.trailers_checked {
						return None;
					}
					self.trailers_checked = true;
					return self.finish_stream().await;
				},
			}
		}
	}

	async fn finish_stream(&mut self) -> Option<Result<M, LdkServerError>> {
		match self.body.trailers().await {
			Ok(Some(trailers)) => {
				if let Some(error) = grpc_error_from_headers(&trailers) {
					return Some(Err(error));
				}
			},
			Ok(None) => {},
			Err(e) => {
				return Some(Err(LdkServerError::new(
					InternalError,
					format!("Failed to read gRPC stream trailers: {}", e),
				)));
			},
		}

		if self.buf.is_empty() {
			None
		} else {
			Some(Err(LdkServerError::new(
				InternalError,
				"gRPC stream ended with an incomplete frame",
			)))
		}
	}
}

fn build_streaming_client(server_cert_pem: &[u8]) -> Result<StreamingClient, String> {
	let mut pem_reader = Cursor::new(server_cert_pem);
	let certs =
		certs(&mut pem_reader).map_err(|e| format!("Failed to parse server certificate: {e}"))?;
	if certs.is_empty() {
		return Err("Failed to parse server certificate: no certificates found in PEM".to_string());
	}

	let mut roots = RootCertStore::empty();
	let (added, _ignored) = roots.add_parsable_certificates(&certs);
	if added == 0 {
		return Err("Failed to build streaming client: certificate was not accepted".to_string());
	}

	let tls_config = ClientConfig::builder()
		.with_safe_defaults()
		.with_root_certificates(roots)
		.with_no_client_auth();
	let connector = HttpsConnectorBuilder::new()
		.with_tls_config(tls_config)
		.https_only()
		.enable_http2()
		.build();

	Ok(HyperClient::builder().http2_only(true).build(connector))
}

#[cfg(test)]
mod tests {
	use super::*;
	use hyper::Body;
	use reqwest::header::HeaderValue;

	#[test]
	fn test_grpc_error_from_headers_ignores_ok_status() {
		let mut headers = HeaderMap::new();
		headers.insert("grpc-status", HeaderValue::from_static("0"));
		assert!(grpc_error_from_headers(&headers).is_none());
	}

	#[test]
	fn test_grpc_error_from_headers_decodes_message() {
		let mut headers = HeaderMap::new();
		headers.insert("grpc-status", HeaderValue::from_static("3"));
		headers.insert("grpc-message", HeaderValue::from_static("bad%20request"));

		let err = grpc_error_from_headers(&headers).unwrap();
		assert_eq!(err.error_code, InvalidRequestError);
		assert_eq!(err.message, "bad request");
	}

	#[test]
	fn test_grpc_code_to_error_marks_unavailable_streams() {
		let err = grpc_code_to_error(GRPC_STATUS_UNAVAILABLE, "server shutting down".to_string());
		assert_eq!(err.error_code, InternalError);
		assert_eq!(err.message, "gRPC stream became unavailable: server shutting down");
	}

	#[tokio::test]
	async fn test_event_stream_surfaces_terminal_grpc_status() {
		let (mut sender, body) = Body::channel();
		let mut trailers = HeaderMap::new();
		trailers.insert("grpc-status", HeaderValue::from_static("14"));
		trailers.insert("grpc-message", HeaderValue::from_static("server%20restarting"));
		sender.send_trailers(trailers).await.unwrap();
		drop(sender);

		let mut stream: EventStream = GrpcStream {
			body,
			buf: Vec::new(),
			trailers_checked: false,
			_marker: std::marker::PhantomData,
		};

		let result = stream.next_message().await.unwrap().unwrap_err();
		assert_eq!(result.error_code, InternalError);
		assert_eq!(result.message, "gRPC stream became unavailable: server restarting");
		assert!(stream.next_message().await.is_none());
	}

	#[test]
	fn test_grpc_code_to_error_all_known_codes() {
		let cases = [
			(GRPC_STATUS_INVALID_ARGUMENT, InvalidRequestError, "msg"),
			(GRPC_STATUS_UNAUTHENTICATED, AuthError, "msg"),
			(GRPC_STATUS_FAILED_PRECONDITION, LightningError, "msg"),
			(GRPC_STATUS_INTERNAL, InternalServerError, "msg"),
		];
		for (code, expected_error_code, msg) in cases {
			let err = grpc_code_to_error(code, msg.to_string());
			assert_eq!(err.error_code, expected_error_code, "wrong mapping for gRPC code {code}");
			assert_eq!(err.message, msg);
		}
	}

	#[test]
	fn test_grpc_code_to_error_unknown_code() {
		let err = grpc_code_to_error(99, "unknown".to_string());
		assert_eq!(err.error_code, InternalError);
	}
}
