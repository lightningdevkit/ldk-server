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
use ldk_server_protos::api::lightning_node_client::LightningNodeClient;
use ldk_server_protos::api::*;
use ldk_server_protos::events::EventEnvelope;
use tonic::metadata::MetadataValue;
use tonic::service::interceptor::InterceptedService;
use tonic::service::Interceptor;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint};
use tonic::{Request, Status};

use crate::error::LdkServerError;
use crate::error::LdkServerErrorCode::{
	AuthError, InternalError, InternalServerError, InvalidRequestError, LightningError,
};

/// Interceptor that computes and attaches HMAC auth metadata to each gRPC request.
#[derive(Clone)]
struct AuthInterceptor {
	api_key: String,
}

impl Interceptor for AuthInterceptor {
	fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
		let timestamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.expect("System time should be after Unix epoch")
			.as_secs();

		let mut hmac_engine: HmacEngine<sha256::Hash> = HmacEngine::new(self.api_key.as_bytes());
		hmac_engine.input(&timestamp.to_be_bytes());
		let hmac_result = Hmac::<sha256::Hash>::from_engine(hmac_engine);

		let auth_value = format!("HMAC {}:{}", timestamp, hmac_result);
		let meta_value = MetadataValue::try_from(&auth_value)
			.map_err(|_| Status::internal("Failed to encode auth header"))?;
		request.metadata_mut().insert("x-auth", meta_value);

		Ok(request)
	}
}

/// Client to access a hosted instance of LDK Server via gRPC.
///
/// The client requires the server's TLS certificate to be provided for verification.
/// This certificate can be found at `<server_storage_dir>/tls.crt` after the
/// server generates it on first startup.
#[derive(Clone)]
pub struct LdkServerClient {
	inner: LightningNodeClient<InterceptedService<Channel, AuthInterceptor>>,
}

impl LdkServerClient {
	/// Constructs a [`LdkServerClient`] using `base_url` as the ldk-server endpoint.
	///
	/// `base_url` should not include the scheme, e.g., `localhost:3000`.
	/// `api_key` is used for HMAC-based authentication.
	/// `server_cert_pem` is the server's TLS certificate in PEM format. This can be
	/// found at `<server_storage_dir>/tls.crt` after the server starts.
	pub async fn new(
		base_url: String, api_key: String, server_cert_pem: &[u8],
	) -> Result<Self, String> {
		let cert = Certificate::from_pem(server_cert_pem);
		let tls_config = ClientTlsConfig::new().ca_certificate(cert);

		let endpoint = Endpoint::from_shared(format!("https://{}", base_url))
			.map_err(|e| format!("Invalid endpoint URL: {e}"))?
			.tls_config(tls_config)
			.map_err(|e| format!("Failed to configure TLS: {e}"))?;

		let channel =
			endpoint.connect().await.map_err(|e| format!("Failed to connect to server: {e}"))?;

		let interceptor = AuthInterceptor { api_key };
		let inner = LightningNodeClient::with_interceptor(channel, interceptor)
			.max_decoding_message_size(10 * 1024 * 1024);

		Ok(Self { inner })
	}

	/// Retrieve the latest node info like `node_id`, `current_best_block` etc.
	/// For API contract/usage, refer to docs for [`GetNodeInfoRequest`] and [`GetNodeInfoResponse`].
	pub async fn get_node_info(
		&self, request: GetNodeInfoRequest,
	) -> Result<GetNodeInfoResponse, LdkServerError> {
		self.inner.clone().get_node_info(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Retrieves an overview of all known balances.
	/// For API contract/usage, refer to docs for [`GetBalancesRequest`] and [`GetBalancesResponse`].
	pub async fn get_balances(
		&self, request: GetBalancesRequest,
	) -> Result<GetBalancesResponse, LdkServerError> {
		self.inner.clone().get_balances(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Retrieve a new on-chain funding address.
	/// For API contract/usage, refer to docs for [`OnchainReceiveRequest`] and [`OnchainReceiveResponse`].
	pub async fn onchain_receive(
		&self, request: OnchainReceiveRequest,
	) -> Result<OnchainReceiveResponse, LdkServerError> {
		self.inner
			.clone()
			.onchain_receive(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Send an on-chain payment to the given address.
	/// For API contract/usage, refer to docs for [`OnchainSendRequest`] and [`OnchainSendResponse`].
	pub async fn onchain_send(
		&self, request: OnchainSendRequest,
	) -> Result<OnchainSendResponse, LdkServerError> {
		self.inner.clone().onchain_send(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Retrieve a new BOLT11 payable invoice.
	/// For API contract/usage, refer to docs for [`Bolt11ReceiveRequest`] and [`Bolt11ReceiveResponse`].
	pub async fn bolt11_receive(
		&self, request: Bolt11ReceiveRequest,
	) -> Result<Bolt11ReceiveResponse, LdkServerError> {
		self.inner
			.clone()
			.bolt11_receive(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Retrieve a new BOLT11 payable invoice for a given payment hash.
	pub async fn bolt11_receive_for_hash(
		&self, request: Bolt11ReceiveForHashRequest,
	) -> Result<Bolt11ReceiveForHashResponse, LdkServerError> {
		self.inner
			.clone()
			.bolt11_receive_for_hash(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Manually claim a payment for a given payment hash with the corresponding preimage.
	pub async fn bolt11_claim_for_hash(
		&self, request: Bolt11ClaimForHashRequest,
	) -> Result<Bolt11ClaimForHashResponse, LdkServerError> {
		self.inner
			.clone()
			.bolt11_claim_for_hash(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Manually fail a payment for a given payment hash.
	pub async fn bolt11_fail_for_hash(
		&self, request: Bolt11FailForHashRequest,
	) -> Result<Bolt11FailForHashResponse, LdkServerError> {
		self.inner
			.clone()
			.bolt11_fail_for_hash(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Retrieve a new fixed-amount BOLT11 invoice for receiving via an LSPS2 JIT channel.
	pub async fn bolt11_receive_via_jit_channel(
		&self, request: Bolt11ReceiveViaJitChannelRequest,
	) -> Result<Bolt11ReceiveViaJitChannelResponse, LdkServerError> {
		self.inner
			.clone()
			.bolt11_receive_via_jit_channel(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Retrieve a new variable-amount BOLT11 invoice for receiving via an LSPS2 JIT channel.
	pub async fn bolt11_receive_variable_amount_via_jit_channel(
		&self, request: Bolt11ReceiveVariableAmountViaJitChannelRequest,
	) -> Result<Bolt11ReceiveVariableAmountViaJitChannelResponse, LdkServerError> {
		self.inner
			.clone()
			.bolt11_receive_variable_amount_via_jit_channel(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Send a payment for a BOLT11 invoice.
	/// For API contract/usage, refer to docs for [`Bolt11SendRequest`] and [`Bolt11SendResponse`].
	pub async fn bolt11_send(
		&self, request: Bolt11SendRequest,
	) -> Result<Bolt11SendResponse, LdkServerError> {
		self.inner.clone().bolt11_send(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Retrieve a new BOLT12 offer.
	/// For API contract/usage, refer to docs for [`Bolt12ReceiveRequest`] and [`Bolt12ReceiveResponse`].
	pub async fn bolt12_receive(
		&self, request: Bolt12ReceiveRequest,
	) -> Result<Bolt12ReceiveResponse, LdkServerError> {
		self.inner
			.clone()
			.bolt12_receive(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Send a payment for a BOLT12 offer.
	/// For API contract/usage, refer to docs for [`Bolt12SendRequest`] and [`Bolt12SendResponse`].
	pub async fn bolt12_send(
		&self, request: Bolt12SendRequest,
	) -> Result<Bolt12SendResponse, LdkServerError> {
		self.inner.clone().bolt12_send(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Creates a new outbound channel.
	/// For API contract/usage, refer to docs for [`OpenChannelRequest`] and [`OpenChannelResponse`].
	pub async fn open_channel(
		&self, request: OpenChannelRequest,
	) -> Result<OpenChannelResponse, LdkServerError> {
		self.inner.clone().open_channel(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Splices funds into the channel specified by given request.
	pub async fn splice_in(
		&self, request: SpliceInRequest,
	) -> Result<SpliceInResponse, LdkServerError> {
		self.inner.clone().splice_in(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Splices funds out of the channel specified by given request.
	pub async fn splice_out(
		&self, request: SpliceOutRequest,
	) -> Result<SpliceOutResponse, LdkServerError> {
		self.inner.clone().splice_out(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Closes the channel specified by given request.
	pub async fn close_channel(
		&self, request: CloseChannelRequest,
	) -> Result<CloseChannelResponse, LdkServerError> {
		self.inner.clone().close_channel(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Force closes the channel specified by given request.
	pub async fn force_close_channel(
		&self, request: ForceCloseChannelRequest,
	) -> Result<ForceCloseChannelResponse, LdkServerError> {
		self.inner
			.clone()
			.force_close_channel(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Retrieves list of known channels.
	pub async fn list_channels(
		&self, request: ListChannelsRequest,
	) -> Result<ListChannelsResponse, LdkServerError> {
		self.inner.clone().list_channels(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Retrieves list of all payments sent or received by us.
	pub async fn list_payments(
		&self, request: ListPaymentsRequest,
	) -> Result<ListPaymentsResponse, LdkServerError> {
		self.inner.clone().list_payments(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Updates the config for a previously opened channel.
	pub async fn update_channel_config(
		&self, request: UpdateChannelConfigRequest,
	) -> Result<UpdateChannelConfigResponse, LdkServerError> {
		self.inner
			.clone()
			.update_channel_config(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Retrieves payment details for a given payment id.
	pub async fn get_payment_details(
		&self, request: GetPaymentDetailsRequest,
	) -> Result<GetPaymentDetailsResponse, LdkServerError> {
		self.inner
			.clone()
			.get_payment_details(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Retrieves list of all forwarded payments.
	pub async fn list_forwarded_payments(
		&self, request: ListForwardedPaymentsRequest,
	) -> Result<ListForwardedPaymentsResponse, LdkServerError> {
		self.inner
			.clone()
			.list_forwarded_payments(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Connect to a peer on the Lightning Network.
	pub async fn connect_peer(
		&self, request: ConnectPeerRequest,
	) -> Result<ConnectPeerResponse, LdkServerError> {
		self.inner.clone().connect_peer(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Disconnect from a peer and remove it from the peer store.
	pub async fn disconnect_peer(
		&self, request: DisconnectPeerRequest,
	) -> Result<DisconnectPeerResponse, LdkServerError> {
		self.inner
			.clone()
			.disconnect_peer(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Retrieves list of peers.
	pub async fn list_peers(
		&self, request: ListPeersRequest,
	) -> Result<ListPeersResponse, LdkServerError> {
		self.inner.clone().list_peers(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Send a spontaneous payment (keysend) to a node.
	pub async fn spontaneous_send(
		&self, request: SpontaneousSendRequest,
	) -> Result<SpontaneousSendResponse, LdkServerError> {
		self.inner
			.clone()
			.spontaneous_send(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Send a payment given a BIP 21 URI or BIP 353 Human-Readable Name.
	pub async fn unified_send(
		&self, request: UnifiedSendRequest,
	) -> Result<UnifiedSendResponse, LdkServerError> {
		self.inner.clone().unified_send(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Decode a BOLT11 invoice and return its parsed fields.
	/// For API contract/usage, refer to docs for [`DecodeInvoiceRequest`] and [`DecodeInvoiceResponse`].
	pub async fn decode_invoice(
		&self, request: DecodeInvoiceRequest,
	) -> Result<DecodeInvoiceResponse, LdkServerError> {
		self.inner
			.clone()
			.decode_invoice(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Decode a BOLT12 offer and return its parsed fields.
	/// For API contract/usage, refer to docs for [`DecodeOfferRequest`] and [`DecodeOfferResponse`].
	pub async fn decode_offer(
		&self, request: DecodeOfferRequest,
	) -> Result<DecodeOfferResponse, LdkServerError> {
		self.inner.clone().decode_offer(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Sign a message with the node's secret key.
	pub async fn sign_message(
		&self, request: SignMessageRequest,
	) -> Result<SignMessageResponse, LdkServerError> {
		self.inner.clone().sign_message(request).await.map(|r| r.into_inner()).map_err(from_status)
	}

	/// Verify a signature against a message and public key.
	pub async fn verify_signature(
		&self, request: VerifySignatureRequest,
	) -> Result<VerifySignatureResponse, LdkServerError> {
		self.inner
			.clone()
			.verify_signature(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Export the pathfinding scores used by the router.
	pub async fn export_pathfinding_scores(
		&self, request: ExportPathfindingScoresRequest,
	) -> Result<ExportPathfindingScoresResponse, LdkServerError> {
		self.inner
			.clone()
			.export_pathfinding_scores(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Returns a list of all known short channel IDs in the network graph.
	pub async fn graph_list_channels(
		&self, request: GraphListChannelsRequest,
	) -> Result<GraphListChannelsResponse, LdkServerError> {
		self.inner
			.clone()
			.graph_list_channels(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Returns information on a channel with the given short channel ID from the network graph.
	pub async fn graph_get_channel(
		&self, request: GraphGetChannelRequest,
	) -> Result<GraphGetChannelResponse, LdkServerError> {
		self.inner
			.clone()
			.graph_get_channel(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Returns a list of all known node IDs in the network graph.
	pub async fn graph_list_nodes(
		&self, request: GraphListNodesRequest,
	) -> Result<GraphListNodesResponse, LdkServerError> {
		self.inner
			.clone()
			.graph_list_nodes(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Returns information on a node with the given ID from the network graph.
	pub async fn graph_get_node(
		&self, request: GraphGetNodeRequest,
	) -> Result<GraphGetNodeResponse, LdkServerError> {
		self.inner
			.clone()
			.graph_get_node(request)
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}

	/// Subscribe to a stream of server events (payments, forwards, etc.).
	pub async fn subscribe_events(
		&self,
	) -> Result<tonic::Streaming<EventEnvelope>, LdkServerError> {
		self.inner
			.clone()
			.subscribe_events(SubscribeEventsRequest {})
			.await
			.map(|r| r.into_inner())
			.map_err(from_status)
	}
}

fn from_status(status: Status) -> LdkServerError {
	let error_code = match status.code() {
		tonic::Code::InvalidArgument => InvalidRequestError,
		tonic::Code::Unauthenticated | tonic::Code::PermissionDenied => AuthError,
		tonic::Code::FailedPrecondition => LightningError,
		tonic::Code::Internal => InternalServerError,
		tonic::Code::Ok
		| tonic::Code::Cancelled
		| tonic::Code::Unknown
		| tonic::Code::DeadlineExceeded
		| tonic::Code::NotFound
		| tonic::Code::AlreadyExists
		| tonic::Code::ResourceExhausted
		| tonic::Code::Aborted
		| tonic::Code::OutOfRange
		| tonic::Code::Unimplemented
		| tonic::Code::Unavailable
		| tonic::Code::DataLoss => InternalError,
	};
	LdkServerError::new(error_code, status.message().to_string())
}
