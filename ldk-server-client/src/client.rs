// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::time::{SystemTime, UNIX_EPOCH};

use bitcoin_hashes::hmac::{Hmac, HmacEngine};
use bitcoin_hashes::{sha256, Hash, HashEngine};
use ldk_server_json_models::api::{
	Bolt11ClaimForHashRequest, Bolt11ClaimForHashResponse, Bolt11FailForHashRequest,
	Bolt11FailForHashResponse, Bolt11ReceiveForHashRequest, Bolt11ReceiveForHashResponse,
	Bolt11ReceiveRequest, Bolt11ReceiveResponse, Bolt11ReceiveVariableAmountViaJitChannelRequest,
	Bolt11ReceiveVariableAmountViaJitChannelResponse, Bolt11ReceiveViaJitChannelRequest,
	Bolt11ReceiveViaJitChannelResponse, Bolt11SendRequest, Bolt11SendResponse,
	Bolt12ReceiveRequest, Bolt12ReceiveResponse, Bolt12SendRequest, Bolt12SendResponse,
	CloseChannelRequest, CloseChannelResponse, ConnectPeerRequest, ConnectPeerResponse,
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
use ldk_server_json_models::endpoints::{
	BOLT11_CLAIM_FOR_HASH_PATH, BOLT11_FAIL_FOR_HASH_PATH, BOLT11_RECEIVE_FOR_HASH_PATH,
	BOLT11_RECEIVE_PATH, BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH,
	BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH, BOLT11_SEND_PATH, BOLT12_RECEIVE_PATH, BOLT12_SEND_PATH,
	CLOSE_CHANNEL_PATH, CONNECT_PEER_PATH, DISCONNECT_PEER_PATH, EXPORT_PATHFINDING_SCORES_PATH,
	FORCE_CLOSE_CHANNEL_PATH, GET_BALANCES_PATH, GET_NODE_INFO_PATH, GET_PAYMENT_DETAILS_PATH,
	GRAPH_GET_CHANNEL_PATH, GRAPH_GET_NODE_PATH, GRAPH_LIST_CHANNELS_PATH, GRAPH_LIST_NODES_PATH,
	LIST_CHANNELS_PATH, LIST_FORWARDED_PAYMENTS_PATH, LIST_PAYMENTS_PATH, LIST_PEERS_PATH,
	ONCHAIN_RECEIVE_PATH, ONCHAIN_SEND_PATH, OPEN_CHANNEL_PATH, SIGN_MESSAGE_PATH, SPLICE_IN_PATH,
	SPLICE_OUT_PATH, SPONTANEOUS_SEND_PATH, SUBSCRIBE_PATH, UNIFIED_SEND_PATH,
	UPDATE_CHANNEL_CONFIG_PATH, VERIFY_SIGNATURE_PATH,
};
use ldk_server_json_models::error::{ErrorCode, ErrorResponse};
use reqwest::header::CONTENT_TYPE;
use reqwest::{Certificate, Client};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::LdkServerError;
use crate::error::LdkServerErrorCode::{
	AuthError, InternalError, InternalServerError, InvalidRequestError, JsonParseError,
	LightningError,
};

const APPLICATION_JSON: &str = "application/json";

/// Client to access a hosted instance of LDK Server.
///
/// The client requires the server's TLS certificate to be provided for verification.
/// This certificate can be found at `<server_storage_dir>/tls.crt` after the
/// server generates it on first startup.
#[derive(Clone)]
pub struct LdkServerClient {
	base_url: String,
	client: Client,
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

		let client = Client::builder()
			.add_root_certificate(cert)
			.build()
			.map_err(|e| format!("Failed to build HTTP client: {e}"))?;

		Ok(Self { base_url, client, api_key })
	}

	/// Computes the HMAC-SHA256 authentication header value.
	/// Format: "HMAC <timestamp>:<hmac_hex>"
	fn compute_auth_header(&self, body: &[u8]) -> String {
		let timestamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.expect("System time should be after Unix epoch")
			.as_secs();

		// Compute HMAC-SHA256(api_key, timestamp_bytes || body)
		let mut hmac_engine: HmacEngine<sha256::Hash> = HmacEngine::new(self.api_key.as_bytes());
		hmac_engine.input(&timestamp.to_be_bytes());
		hmac_engine.input(body);
		let hmac_result = Hmac::<sha256::Hash>::from_engine(hmac_engine);

		format!("HMAC {}:{}", timestamp, hmac_result)
	}

	/// Retrieve the latest node info like `node_id`, `current_best_block` etc.
	/// For API contract/usage, refer to docs for [`GetNodeInfoRequest`] and [`GetNodeInfoResponse`].
	pub async fn get_node_info(
		&self, request: GetNodeInfoRequest,
	) -> Result<GetNodeInfoResponse, LdkServerError> {
		let url = format!("https://{}/{GET_NODE_INFO_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Retrieves an overview of all known balances.
	/// For API contract/usage, refer to docs for [`GetBalancesRequest`] and [`GetBalancesResponse`].
	pub async fn get_balances(
		&self, request: GetBalancesRequest,
	) -> Result<GetBalancesResponse, LdkServerError> {
		let url = format!("https://{}/{GET_BALANCES_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Retrieve a new on-chain funding address.
	/// For API contract/usage, refer to docs for [`OnchainReceiveRequest`] and [`OnchainReceiveResponse`].
	pub async fn onchain_receive(
		&self, request: OnchainReceiveRequest,
	) -> Result<OnchainReceiveResponse, LdkServerError> {
		let url = format!("https://{}/{ONCHAIN_RECEIVE_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Send an on-chain payment to the given address.
	/// For API contract/usage, refer to docs for [`OnchainSendRequest`] and [`OnchainSendResponse`].
	pub async fn onchain_send(
		&self, request: OnchainSendRequest,
	) -> Result<OnchainSendResponse, LdkServerError> {
		let url = format!("https://{}/{ONCHAIN_SEND_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Retrieve a new BOLT11 payable invoice.
	/// For API contract/usage, refer to docs for [`Bolt11ReceiveRequest`] and [`Bolt11ReceiveResponse`].
	pub async fn bolt11_receive(
		&self, request: Bolt11ReceiveRequest,
	) -> Result<Bolt11ReceiveResponse, LdkServerError> {
		let url = format!("https://{}/{BOLT11_RECEIVE_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Retrieve a new BOLT11 payable invoice for a given payment hash.
	/// The inbound payment will NOT be automatically claimed upon arrival.
	/// For API contract/usage, refer to docs for [`Bolt11ReceiveForHashRequest`] and [`Bolt11ReceiveForHashResponse`].
	pub async fn bolt11_receive_for_hash(
		&self, request: Bolt11ReceiveForHashRequest,
	) -> Result<Bolt11ReceiveForHashResponse, LdkServerError> {
		let url = format!("https://{}/{BOLT11_RECEIVE_FOR_HASH_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Manually claim a payment for a given payment hash with the corresponding preimage.
	/// For API contract/usage, refer to docs for [`Bolt11ClaimForHashRequest`] and [`Bolt11ClaimForHashResponse`].
	pub async fn bolt11_claim_for_hash(
		&self, request: Bolt11ClaimForHashRequest,
	) -> Result<Bolt11ClaimForHashResponse, LdkServerError> {
		let url = format!("https://{}/{BOLT11_CLAIM_FOR_HASH_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Manually fail a payment for a given payment hash.
	/// For API contract/usage, refer to docs for [`Bolt11FailForHashRequest`] and [`Bolt11FailForHashResponse`].
	pub async fn bolt11_fail_for_hash(
		&self, request: Bolt11FailForHashRequest,
	) -> Result<Bolt11FailForHashResponse, LdkServerError> {
		let url = format!("https://{}/{BOLT11_FAIL_FOR_HASH_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Retrieve a new fixed-amount BOLT11 invoice for receiving via an LSPS2 JIT channel.
	/// For API contract/usage, refer to docs for [`Bolt11ReceiveViaJitChannelRequest`] and
	/// [`Bolt11ReceiveViaJitChannelResponse`].
	pub async fn bolt11_receive_via_jit_channel(
		&self, request: Bolt11ReceiveViaJitChannelRequest,
	) -> Result<Bolt11ReceiveViaJitChannelResponse, LdkServerError> {
		let url = format!("https://{}/{BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Retrieve a new variable-amount BOLT11 invoice for receiving via an LSPS2 JIT channel.
	/// For API contract/usage, refer to docs for
	/// [`Bolt11ReceiveVariableAmountViaJitChannelRequest`] and
	/// [`Bolt11ReceiveVariableAmountViaJitChannelResponse`].
	pub async fn bolt11_receive_variable_amount_via_jit_channel(
		&self, request: Bolt11ReceiveVariableAmountViaJitChannelRequest,
	) -> Result<Bolt11ReceiveVariableAmountViaJitChannelResponse, LdkServerError> {
		let url = format!(
			"https://{}/{BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH}",
			self.base_url,
		);
		self.post_request(&request, &url).await
	}

	/// Send a payment for a BOLT11 invoice.
	/// For API contract/usage, refer to docs for [`Bolt11SendRequest`] and [`Bolt11SendResponse`].
	pub async fn bolt11_send(
		&self, request: Bolt11SendRequest,
	) -> Result<Bolt11SendResponse, LdkServerError> {
		let url = format!("https://{}/{BOLT11_SEND_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Retrieve a new BOLT11 payable offer.
	/// For API contract/usage, refer to docs for [`Bolt12ReceiveRequest`] and [`Bolt12ReceiveResponse`].
	pub async fn bolt12_receive(
		&self, request: Bolt12ReceiveRequest,
	) -> Result<Bolt12ReceiveResponse, LdkServerError> {
		let url = format!("https://{}/{BOLT12_RECEIVE_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Send a payment for a BOLT12 offer.
	/// For API contract/usage, refer to docs for [`Bolt12SendRequest`] and [`Bolt12SendResponse`].
	pub async fn bolt12_send(
		&self, request: Bolt12SendRequest,
	) -> Result<Bolt12SendResponse, LdkServerError> {
		let url = format!("https://{}/{BOLT12_SEND_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Creates a new outbound channel.
	/// For API contract/usage, refer to docs for [`OpenChannelRequest`] and [`OpenChannelResponse`].
	pub async fn open_channel(
		&self, request: OpenChannelRequest,
	) -> Result<OpenChannelResponse, LdkServerError> {
		let url = format!("https://{}/{OPEN_CHANNEL_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Splices funds into the channel specified by given request.
	/// For API contract/usage, refer to docs for [`SpliceInRequest`] and [`SpliceInResponse`].
	pub async fn splice_in(
		&self, request: SpliceInRequest,
	) -> Result<SpliceInResponse, LdkServerError> {
		let url = format!("https://{}/{SPLICE_IN_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Splices funds out of the channel specified by given request.
	/// For API contract/usage, refer to docs for [`SpliceOutRequest`] and [`SpliceOutResponse`].
	pub async fn splice_out(
		&self, request: SpliceOutRequest,
	) -> Result<SpliceOutResponse, LdkServerError> {
		let url = format!("https://{}/{SPLICE_OUT_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Closes the channel specified by given request.
	/// For API contract/usage, refer to docs for [`CloseChannelRequest`] and [`CloseChannelResponse`].
	pub async fn close_channel(
		&self, request: CloseChannelRequest,
	) -> Result<CloseChannelResponse, LdkServerError> {
		let url = format!("https://{}/{CLOSE_CHANNEL_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Force closes the channel specified by given request.
	/// For API contract/usage, refer to docs for [`ForceCloseChannelRequest`] and [`ForceCloseChannelResponse`].
	pub async fn force_close_channel(
		&self, request: ForceCloseChannelRequest,
	) -> Result<ForceCloseChannelResponse, LdkServerError> {
		let url = format!("https://{}/{FORCE_CLOSE_CHANNEL_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Retrieves list of known channels.
	/// For API contract/usage, refer to docs for [`ListChannelsRequest`] and [`ListChannelsResponse`].
	pub async fn list_channels(
		&self, request: ListChannelsRequest,
	) -> Result<ListChannelsResponse, LdkServerError> {
		let url = format!("https://{}/{LIST_CHANNELS_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Retrieves list of all payments sent or received by us.
	/// For API contract/usage, refer to docs for [`ListPaymentsRequest`] and [`ListPaymentsResponse`].
	pub async fn list_payments(
		&self, request: ListPaymentsRequest,
	) -> Result<ListPaymentsResponse, LdkServerError> {
		let url = format!("https://{}/{LIST_PAYMENTS_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Updates the config for a previously opened channel.
	/// For API contract/usage, refer to docs for [`UpdateChannelConfigRequest`] and [`UpdateChannelConfigResponse`].
	pub async fn update_channel_config(
		&self, request: UpdateChannelConfigRequest,
	) -> Result<UpdateChannelConfigResponse, LdkServerError> {
		let url = format!("https://{}/{UPDATE_CHANNEL_CONFIG_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Retrieves payment details for a given payment id.
	/// For API contract/usage, refer to docs for [`GetPaymentDetailsRequest`] and [`GetPaymentDetailsResponse`].
	pub async fn get_payment_details(
		&self, request: GetPaymentDetailsRequest,
	) -> Result<GetPaymentDetailsResponse, LdkServerError> {
		let url = format!("https://{}/{GET_PAYMENT_DETAILS_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Retrieves list of all forwarded payments.
	/// For API contract/usage, refer to docs for [`ListForwardedPaymentsRequest`] and [`ListForwardedPaymentsResponse`].
	pub async fn list_forwarded_payments(
		&self, request: ListForwardedPaymentsRequest,
	) -> Result<ListForwardedPaymentsResponse, LdkServerError> {
		let url = format!("https://{}/{LIST_FORWARDED_PAYMENTS_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Connect to a peer on the Lightning Network.
	/// For API contract/usage, refer to docs for [`ConnectPeerRequest`] and [`ConnectPeerResponse`].
	pub async fn connect_peer(
		&self, request: ConnectPeerRequest,
	) -> Result<ConnectPeerResponse, LdkServerError> {
		let url = format!("https://{}/{CONNECT_PEER_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Disconnect from a peer and remove it from the peer store.
	/// For API contract/usage, refer to docs for [`DisconnectPeerRequest`] and [`DisconnectPeerResponse`].
	pub async fn disconnect_peer(
		&self, request: DisconnectPeerRequest,
	) -> Result<DisconnectPeerResponse, LdkServerError> {
		let url = format!("https://{}/{DISCONNECT_PEER_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Retrieves list of peers.
	/// For API contract/usage, refer to docs for [`ListPeersRequest`] and [`ListPeersResponse`].
	pub async fn list_peers(
		&self, request: ListPeersRequest,
	) -> Result<ListPeersResponse, LdkServerError> {
		let url = format!("https://{}/{LIST_PEERS_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Send a spontaneous payment (keysend) to a node.
	/// For API contract/usage, refer to docs for [`SpontaneousSendRequest`] and [`SpontaneousSendResponse`].
	pub async fn spontaneous_send(
		&self, request: SpontaneousSendRequest,
	) -> Result<SpontaneousSendResponse, LdkServerError> {
		let url = format!("https://{}/{SPONTANEOUS_SEND_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Send a payment given a BIP 21 URI or BIP 353 Human-Readable Name.
	/// For API contract/usage, refer to docs for [`UnifiedSendRequest`] and [`UnifiedSendResponse`].
	pub async fn unified_send(
		&self, request: UnifiedSendRequest,
	) -> Result<UnifiedSendResponse, LdkServerError> {
		let url = format!("https://{}/{UNIFIED_SEND_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Sign a message with the node's secret key.
	/// For API contract/usage, refer to docs for [`SignMessageRequest`] and [`SignMessageResponse`].
	pub async fn sign_message(
		&self, request: SignMessageRequest,
	) -> Result<SignMessageResponse, LdkServerError> {
		let url = format!("https://{}/{SIGN_MESSAGE_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Verify a signature against a message and public key.
	/// For API contract/usage, refer to docs for [`VerifySignatureRequest`] and [`VerifySignatureResponse`].
	pub async fn verify_signature(
		&self, request: VerifySignatureRequest,
	) -> Result<VerifySignatureResponse, LdkServerError> {
		let url = format!("https://{}/{VERIFY_SIGNATURE_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Export the pathfinding scores used by the router.
	/// For API contract/usage, refer to docs for [`ExportPathfindingScoresRequest`] and [`ExportPathfindingScoresResponse`].
	pub async fn export_pathfinding_scores(
		&self, request: ExportPathfindingScoresRequest,
	) -> Result<ExportPathfindingScoresResponse, LdkServerError> {
		let url = format!("https://{}/{EXPORT_PATHFINDING_SCORES_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Returns a list of all known short channel IDs in the network graph.
	/// For API contract/usage, refer to docs for [`GraphListChannelsRequest`] and [`GraphListChannelsResponse`].
	pub async fn graph_list_channels(
		&self, request: GraphListChannelsRequest,
	) -> Result<GraphListChannelsResponse, LdkServerError> {
		let url = format!("https://{}/{GRAPH_LIST_CHANNELS_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Returns information on a channel with the given short channel ID from the network graph.
	/// For API contract/usage, refer to docs for [`GraphGetChannelRequest`] and [`GraphGetChannelResponse`].
	pub async fn graph_get_channel(
		&self, request: GraphGetChannelRequest,
	) -> Result<GraphGetChannelResponse, LdkServerError> {
		let url = format!("https://{}/{GRAPH_GET_CHANNEL_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Returns a list of all known node IDs in the network graph.
	/// For API contract/usage, refer to docs for [`GraphListNodesRequest`] and [`GraphListNodesResponse`].
	pub async fn graph_list_nodes(
		&self, request: GraphListNodesRequest,
	) -> Result<GraphListNodesResponse, LdkServerError> {
		let url = format!("https://{}/{GRAPH_LIST_NODES_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Returns information on a node with the given ID from the network graph.
	/// For API contract/usage, refer to docs for [`GraphGetNodeRequest`] and [`GraphGetNodeResponse`].
	pub async fn graph_get_node(
		&self, request: GraphGetNodeRequest,
	) -> Result<GraphGetNodeResponse, LdkServerError> {
		let url = format!("https://{}/{GRAPH_GET_NODE_PATH}", self.base_url);
		self.post_request(&request, &url).await
	}

	/// Subscribe to server-sent events. Returns an async stream of [`Event`] values.
	///
	/// The stream yields one item per SSE event. It ends when the server closes the connection.
	///
	/// [`Event`]: ldk_server_json_models::events::Event
	pub async fn subscribe(
		&self,
	) -> Result<
		impl futures_util::Stream<Item = ldk_server_json_models::events::Event>,
		LdkServerError,
	> {
		use futures_util::StreamExt;
		let url = format!("https://{}/{SUBSCRIBE_PATH}", self.base_url);
		let auth_header = self.compute_auth_header(&[]);
		let response = self
			.client
			.get(&url)
			.header("X-Auth", auth_header)
			.header("Accept", "text/event-stream")
			.send()
			.await
			.map_err(|e| {
				LdkServerError::new(InternalError, format!("HTTP request failed: {}", e))
			})?;

		if !response.status().is_success() {
			let status = response.status();
			let payload = response.bytes().await.map_err(|e| {
				LdkServerError::new(InternalError, format!("Failed to read response body: {}", e))
			})?;
			let error_response =
				serde_json::from_slice::<ErrorResponse>(&payload).map_err(|e| {
					LdkServerError::new(
						JsonParseError,
						format!("Failed to decode error response (status {}): {}", status, e),
					)
				})?;
			let error_code = match error_response.error_code {
				ErrorCode::InvalidRequestError => InvalidRequestError,
				ErrorCode::AuthError => AuthError,
				ErrorCode::LightningError => LightningError,
				ErrorCode::InternalServerError => InternalServerError,
				ErrorCode::UnknownError => InternalError,
			};
			return Err(LdkServerError::new(error_code, error_response.message));
		}

		let stream = async_stream::stream! {
			let mut byte_stream = response.bytes_stream();
			let mut buffer = String::new();
			while let Some(chunk) = byte_stream.next().await {
				let chunk = match chunk {
					Ok(c) => c,
					Err(_) => break,
				};
				buffer.push_str(&String::from_utf8_lossy(&chunk));
				while let Some(pos) = buffer.find("\n\n") {
					let event_block = buffer[..pos].to_string();
					buffer = buffer[pos + 2..].to_string();
					for line in event_block.lines() {
						if let Some(data) = line.strip_prefix("data: ") {
							if let Ok(event) = serde_json::from_str::<ldk_server_json_models::events::Event>(data) {
								yield event;
							}
						}
					}
				}
			}
		};

		Ok(stream)
	}

	async fn post_request<Rq: Serialize, Rs: DeserializeOwned>(
		&self, request: &Rq, url: &str,
	) -> Result<Rs, LdkServerError> {
		let request_body = serde_json::to_vec(request).map_err(|e| {
			LdkServerError::new(JsonParseError, format!("Failed to serialize request: {}", e))
		})?;
		let auth_header = self.compute_auth_header(&request_body);
		let response_raw = self
			.client
			.post(url)
			.header(CONTENT_TYPE, APPLICATION_JSON)
			.header("X-Auth", auth_header)
			.body(request_body)
			.send()
			.await
			.map_err(|e| {
				LdkServerError::new(InternalError, format!("HTTP request failed: {}", e))
			})?;

		let status = response_raw.status();
		let payload = response_raw.bytes().await.map_err(|e| {
			LdkServerError::new(InternalError, format!("Failed to read response body: {}", e))
		})?;

		if status.is_success() {
			Ok(serde_json::from_slice::<Rs>(&payload).map_err(|e| {
				LdkServerError::new(
					JsonParseError,
					format!("Failed to decode success response: {}", e),
				)
			})?)
		} else {
			let error_response =
				serde_json::from_slice::<ErrorResponse>(&payload).map_err(|e| {
					LdkServerError::new(
						JsonParseError,
						format!("Failed to decode error response (status {}): {}", status, e),
					)
				})?;

			let error_code = match error_response.error_code {
				ErrorCode::InvalidRequestError => InvalidRequestError,
				ErrorCode::AuthError => AuthError,
				ErrorCode::LightningError => LightningError,
				ErrorCode::InternalServerError => InternalServerError,
				ErrorCode::UnknownError => InternalError,
			};

			Err(LdkServerError::new(error_code, error_response.message))
		}
	}
}
