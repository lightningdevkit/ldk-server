// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use http_body_util::{BodyExt, Limited};
use hyper::body::Incoming;
use hyper::service::Service;
use hyper::{Request, Response};
use ldk_node::bitcoin::hashes::hmac::{Hmac, HmacEngine};
use ldk_node::bitcoin::hashes::{sha256, Hash, HashEngine};
use ldk_node::Node;
use ldk_server_grpc::endpoints::{
	BOLT11_CLAIM_FOR_HASH_PATH, BOLT11_FAIL_FOR_HASH_PATH, BOLT11_RECEIVE_FOR_HASH_PATH,
	BOLT11_RECEIVE_PATH, BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH,
	BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH, BOLT11_SEND_PATH, BOLT12_RECEIVE_PATH, BOLT12_SEND_PATH,
	CLOSE_CHANNEL_PATH, CONNECT_PEER_PATH, DECODE_INVOICE_PATH, DECODE_OFFER_PATH,
	DISCONNECT_PEER_PATH, EXPORT_PATHFINDING_SCORES_PATH, FORCE_CLOSE_CHANNEL_PATH,
	GET_BALANCES_PATH, GET_METRICS_PATH, GET_NODE_INFO_PATH, GET_PAYMENT_DETAILS_PATH,
	GRAPH_GET_CHANNEL_PATH, GRAPH_GET_NODE_PATH, GRAPH_LIST_CHANNELS_PATH, GRAPH_LIST_NODES_PATH,
	LIST_CHANNELS_PATH, LIST_FORWARDED_PAYMENTS_PATH, LIST_PAYMENTS_PATH, LIST_PEERS_PATH,
	ONCHAIN_RECEIVE_PATH, ONCHAIN_SEND_PATH, OPEN_CHANNEL_PATH, SIGN_MESSAGE_PATH, SPLICE_IN_PATH,
	SPLICE_OUT_PATH, SPONTANEOUS_SEND_PATH, SUBSCRIBE_EVENTS_PATH, UNIFIED_SEND_PATH,
	UPDATE_CHANNEL_CONFIG_PATH, VERIFY_SIGNATURE_PATH,
};
use ldk_server_grpc::events::EventEnvelope;
use ldk_server_grpc::grpc::{
	decode_grpc_body, encode_grpc_frame, grpc_error_response, grpc_response, parse_grpc_timeout,
	validate_grpc_request, GrpcBody, GrpcStatus, GRPC_STATUS_DEADLINE_EXCEEDED,
	GRPC_STATUS_FAILED_PRECONDITION, GRPC_STATUS_INTERNAL, GRPC_STATUS_INVALID_ARGUMENT,
	GRPC_STATUS_UNAUTHENTICATED, GRPC_STATUS_UNAVAILABLE, GRPC_STATUS_UNIMPLEMENTED,
};
use prost::Message;
use tokio::sync::{broadcast, mpsc};

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
use crate::util::metrics::Metrics;

/// gRPC path prefix for the LightningNode service.
const GRPC_SERVICE_PREFIX: &str = "/api.LightningNode/";

// Maximum request body size: 10 MB
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct NodeService {
	context: Arc<Context>,
	api_key: String,
	metrics: Option<Arc<Metrics>>,
	metrics_auth_header: Option<String>,
	event_sender: broadcast::Sender<EventEnvelope>,
	shutdown_rx: tokio::sync::watch::Receiver<bool>,
}

impl NodeService {
	pub(crate) fn new(
		node: Arc<Node>, paginated_kv_store: Arc<dyn PaginatedKVStore>, api_key: String,
		metrics: Option<Arc<Metrics>>, metrics_auth_header: Option<String>,
		event_sender: broadcast::Sender<EventEnvelope>,
		shutdown_rx: tokio::sync::watch::Receiver<bool>,
	) -> Self {
		let context = Arc::new(Context { node, paginated_kv_store });
		Self { context, api_key, metrics, metrics_auth_header, event_sender, shutdown_rx }
	}
}

// Maximum allowed time difference between client timestamp and server time (1 minute)
const AUTH_TIMESTAMP_TOLERANCE_SECS: u64 = 60;

/// Validates HMAC authentication from request headers.
/// Uses timestamp-only HMAC (no body) since TLS guarantees integrity.
fn validate_auth<B>(req: &Request<B>, api_key: &str) -> Result<(), LdkServerError> {
	let auth_err = |msg: &str| LdkServerError::new(LdkServerErrorCode::AuthError, msg.to_string());

	let auth_header = req
		.headers()
		.get("x-auth")
		.and_then(|v| v.to_str().ok())
		.ok_or_else(|| auth_err("Missing x-auth metadata"))?;

	let auth_data =
		auth_header.strip_prefix("HMAC ").ok_or_else(|| auth_err("Invalid x-auth format"))?;

	let (timestamp_str, provided_hmac_hex) =
		auth_data.split_once(':').ok_or_else(|| auth_err("Invalid x-auth format"))?;

	let timestamp = timestamp_str.parse::<u64>().map_err(|_| auth_err("Invalid timestamp"))?;

	let now = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map_err(|_| auth_err("System time error"))?
		.as_secs();

	if now.abs_diff(timestamp) > AUTH_TIMESTAMP_TOLERANCE_SECS {
		return Err(auth_err("Request timestamp expired"));
	}

	// HMAC-SHA256(api_key, timestamp_bytes) — no body since TLS guarantees integrity
	let mut hmac_engine: HmacEngine<sha256::Hash> = HmacEngine::new(api_key.as_bytes());
	hmac_engine.input(&timestamp.to_be_bytes());
	let expected_hmac = Hmac::<sha256::Hash>::from_engine(hmac_engine);

	let provided_hmac = provided_hmac_hex
		.parse::<Hmac<sha256::Hash>>()
		.map_err(|_| auth_err("Invalid HMAC in x-auth"))?;

	if expected_hmac != provided_hmac {
		return Err(auth_err("Invalid credentials"));
	}

	Ok(())
}

pub(crate) struct Context {
	pub(crate) node: Arc<Node>,
	pub(crate) paginated_kv_store: Arc<dyn PaginatedKVStore>,
}

impl Service<Request<Incoming>> for NodeService {
	type Response = Response<GrpcBody>;
	type Error = hyper::Error;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

	fn call(&self, req: Request<Incoming>) -> Self::Future {
		// Handle metrics endpoint (plain HTTP GET, not gRPC)
		if req.method() == hyper::Method::GET
			&& req.uri().path().len() > 1
			&& &req.uri().path()[1..] == GET_METRICS_PATH
		{
			if let Some(expected_header) = &self.metrics_auth_header {
				let auth_header = req.headers().get("authorization").and_then(|h| h.to_str().ok());
				if auth_header != Some(expected_header) {
					return Box::pin(async move {
						Ok(Response::builder()
							.status(401)
							.header("www-authenticate", "Basic realm=\"metrics\"")
							.body(GrpcBody::Plain {
								data: Some(bytes::Bytes::from("Unauthorized")),
							})
							.unwrap())
					});
				}
			}

			if let Some(metrics) = &self.metrics {
				let metrics = Arc::clone(metrics);
				return Box::pin(async move {
					Ok(Response::builder()
						.header("content-type", "text/plain")
						.body(GrpcBody::Plain {
							data: Some(bytes::Bytes::from(metrics.gather_metrics())),
						})
						.unwrap())
				});
			} else {
				return Box::pin(async move {
					Ok(Response::builder()
						.status(404)
						.body(GrpcBody::Plain { data: Some(bytes::Bytes::from("Not Found")) })
						.unwrap())
				});
			}
		}

		// Validate gRPC prerequisites
		if let Err(status) = validate_grpc_request(&req) {
			return Box::pin(async move { Ok(grpc_error_response(status)) });
		}

		// Validate auth before reading the body
		if let Err(e) = validate_auth(&req, &self.api_key) {
			let status = ldk_error_to_grpc_status(e);
			return Box::pin(async move { Ok(grpc_error_response(status)) });
		}

		let context = Arc::clone(&self.context);
		let path = req.uri().path().to_string();
		let deadline = match req.headers().get("grpc-timeout") {
			Some(value) => {
				let value = match value.to_str() {
					Ok(value) => value,
					Err(_) => {
						let status = GrpcStatus::new(
							GRPC_STATUS_INVALID_ARGUMENT,
							"Invalid grpc-timeout header",
						);
						return Box::pin(async move { Ok(grpc_error_response(status)) });
					},
				};

				match parse_grpc_timeout(value) {
					Ok(timeout) => Some(timeout),
					Err(status) => return Box::pin(async move { Ok(grpc_error_response(status)) }),
				}
			},
			None => None,
		};

		// Strip the service prefix to get the method name
		let method = match path.strip_prefix(GRPC_SERVICE_PREFIX) {
			Some(m) => m.to_string(),
			None => {
				let status =
					GrpcStatus::new(GRPC_STATUS_UNIMPLEMENTED, format!("Unknown path: {path}"));
				return Box::pin(async move { Ok(grpc_error_response(status)) });
			},
		};

		let is_streaming = false;
		let future: Self::Future = match method.as_str() {
			GET_NODE_INFO_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_get_node_info_request))
			},
			GET_BALANCES_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_get_balances_request))
			},
			ONCHAIN_RECEIVE_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_onchain_receive_request))
			},
			ONCHAIN_SEND_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_onchain_send_request))
			},
			BOLT11_RECEIVE_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_bolt11_receive_request))
			},
			BOLT11_RECEIVE_FOR_HASH_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_bolt11_receive_for_hash_request))
			},
			BOLT11_CLAIM_FOR_HASH_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_bolt11_claim_for_hash_request))
			},
			BOLT11_FAIL_FOR_HASH_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_bolt11_fail_for_hash_request))
			},
			BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH => Box::pin(handle_grpc_unary(
				context,
				req,
				handle_bolt11_receive_via_jit_channel_request,
			)),
			BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH => Box::pin(handle_grpc_unary(
				context,
				req,
				handle_bolt11_receive_variable_amount_via_jit_channel_request,
			)),
			BOLT11_SEND_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_bolt11_send_request))
			},
			BOLT12_RECEIVE_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_bolt12_receive_request))
			},
			BOLT12_SEND_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_bolt12_send_request))
			},
			OPEN_CHANNEL_PATH => Box::pin(handle_grpc_unary(context, req, handle_open_channel)),
			SPLICE_IN_PATH => Box::pin(handle_grpc_unary(context, req, handle_splice_in_request)),
			SPLICE_OUT_PATH => Box::pin(handle_grpc_unary(context, req, handle_splice_out_request)),
			CLOSE_CHANNEL_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_close_channel_request))
			},
			FORCE_CLOSE_CHANNEL_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_force_close_channel_request))
			},
			LIST_CHANNELS_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_list_channels_request))
			},
			UPDATE_CHANNEL_CONFIG_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_update_channel_config_request))
			},
			GET_PAYMENT_DETAILS_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_get_payment_details_request))
			},
			LIST_PAYMENTS_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_list_payments_request))
			},
			LIST_FORWARDED_PAYMENTS_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_list_forwarded_payments_request))
			},
			CONNECT_PEER_PATH => Box::pin(handle_grpc_unary(context, req, handle_connect_peer)),
			DISCONNECT_PEER_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_disconnect_peer))
			},
			LIST_PEERS_PATH => Box::pin(handle_grpc_unary(context, req, handle_list_peers_request)),
			SPONTANEOUS_SEND_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_spontaneous_send_request))
			},
			UNIFIED_SEND_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_unified_send_request))
			},
			SIGN_MESSAGE_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_sign_message_request))
			},
			VERIFY_SIGNATURE_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_verify_signature_request))
			},
			EXPORT_PATHFINDING_SCORES_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_export_pathfinding_scores_request))
			},
			GRAPH_LIST_CHANNELS_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_graph_list_channels_request))
			},
			GRAPH_GET_CHANNEL_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_graph_get_channel_request))
			},
			GRAPH_LIST_NODES_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_graph_list_nodes_request))
			},
			GRAPH_GET_NODE_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_graph_get_node_request))
			},
			DECODE_INVOICE_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_decode_invoice_request))
			},
			DECODE_OFFER_PATH => {
				Box::pin(handle_grpc_unary(context, req, handle_decode_offer_request))
			},
			SUBSCRIBE_EVENTS_PATH => {
				let event_sender = self.event_sender.clone();
				let mut shutdown_rx = self.shutdown_rx.clone();
				Box::pin(async move {
					let mut rx = event_sender.subscribe();
					let (tx, mpsc_rx) = mpsc::channel::<Result<bytes::Bytes, GrpcStatus>>(64);
					tokio::spawn(async move {
						loop {
							tokio::select! {
								biased;
								_ = shutdown_rx.changed() => {
									let _ = tx
										.send(Err(GrpcStatus::new(
											GRPC_STATUS_UNAVAILABLE,
											"server shutting down",
										)))
										.await;
									break;
								},
								result = rx.recv() => {
									match result {
										Ok(event) => {
											let frame = encode_grpc_frame(&event.encode_to_vec());
											if tx.send(Ok(frame)).await.is_err() {
												break; // client disconnected
											}
										},
										Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
											continue; // skip missed events, keep streaming
										},
										Err(tokio::sync::broadcast::error::RecvError::Closed) => {
											let _ = tx
												.send(Err(GrpcStatus::new(
													GRPC_STATUS_UNAVAILABLE,
													"server shutting down",
												)))
												.await;
											break;
										},
									}
								},
							}
						}
					});
					Ok(grpc_response(GrpcBody::Stream { rx: mpsc_rx, done: false }))
				})
			},
			_ => {
				let status =
					GrpcStatus::new(GRPC_STATUS_UNIMPLEMENTED, format!("Unknown method: {method}"));
				Box::pin(async move { Ok(grpc_error_response(status)) })
			},
		};

		// Apply grpc-timeout deadline to unary RPCs (not streaming).
		match deadline {
			Some(d) if !is_streaming => Box::pin(async move {
				tokio::time::timeout(d, future).await.unwrap_or_else(|_| {
					Ok(grpc_error_response(GrpcStatus::new(
						GRPC_STATUS_DEADLINE_EXCEEDED,
						"Deadline exceeded",
					)))
				})
			}),
			_ => future,
		}
	}
}

async fn handle_grpc_unary<
	T: Message + Default,
	R: Message,
	Fut: Future<Output = Result<R, LdkServerError>> + Send,
	F: Fn(Arc<Context>, T) -> Fut + Send,
>(
	context: Arc<Context>, request: Request<Incoming>, handler: F,
) -> Result<Response<GrpcBody>, hyper::Error> {
	// Read and size-limit the request body
	let limited_body = Limited::new(request.into_body(), MAX_BODY_SIZE);
	let bytes = match limited_body.collect().await {
		Ok(collected) => collected.to_bytes(),
		Err(_) => {
			return Ok(grpc_error_response(GrpcStatus::new(
				GRPC_STATUS_INVALID_ARGUMENT,
				"Request body too large or failed to read",
			)));
		},
	};

	// Decode gRPC framing then protobuf
	let req_msg = decode_grpc_body(&bytes)
		.and_then(|b| {
			T::decode(b)
				.map_err(|_| GrpcStatus::new(GRPC_STATUS_INVALID_ARGUMENT, "Malformed request"))
		})
		.map_err(grpc_error_response);
	let req_msg = match req_msg {
		Ok(m) => m,
		Err(resp) => return Ok(resp),
	};

	// Yield before handler execution to allow cancellation if the client
	// has already disconnected (e.g., RST_STREAM). Hyper drops the handler
	// future at yield points when a stream is reset.
	tokio::task::yield_now().await;

	// Call handler
	match handler(context, req_msg).await {
		Ok(response) => {
			let encoded = encode_grpc_frame(&response.encode_to_vec());
			Ok(grpc_response(GrpcBody::Unary { data: Some(encoded), trailers_sent: false }))
		},
		Err(e) => Ok(grpc_error_response(ldk_error_to_grpc_status(e))),
	}
}

/// Map an `LdkServerError` to a `GrpcStatus`.
pub(crate) fn ldk_error_to_grpc_status(e: LdkServerError) -> GrpcStatus {
	let code = match e.error_code {
		LdkServerErrorCode::InvalidRequestError => GRPC_STATUS_INVALID_ARGUMENT,
		LdkServerErrorCode::AuthError => GRPC_STATUS_UNAUTHENTICATED,
		LdkServerErrorCode::LightningError => GRPC_STATUS_FAILED_PRECONDITION,
		LdkServerErrorCode::InternalServerError => GRPC_STATUS_INTERNAL,
	};
	GrpcStatus { code, message: e.message }
}

#[cfg(test)]
mod tests {
	use super::*;

	fn compute_hmac(api_key: &str, timestamp: u64) -> String {
		let mut hmac_engine: HmacEngine<sha256::Hash> = HmacEngine::new(api_key.as_bytes());
		hmac_engine.input(&timestamp.to_be_bytes());
		Hmac::<sha256::Hash>::from_engine(hmac_engine).to_string()
	}

	fn create_test_request(auth_header: Option<String>) -> Request<()> {
		let mut builder =
			Request::builder().method("POST").header("content-type", "application/grpc+proto");
		if let Some(header) = auth_header {
			builder = builder.header("x-auth", header);
		}
		builder.body(()).unwrap()
	}

	#[test]
	fn test_validate_auth_success() {
		let api_key = "test_api_key";
		let timestamp =
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
		let hmac = compute_hmac(api_key, timestamp);
		let auth_header = format!("HMAC {timestamp}:{hmac}");
		let req = create_test_request(Some(auth_header));

		assert!(validate_auth(&req, api_key).is_ok());
	}

	#[test]
	fn test_validate_auth_missing_header() {
		let req = create_test_request(None);
		let result = validate_auth(&req, "test_key");
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().error_code, LdkServerErrorCode::AuthError);
	}

	#[test]
	fn test_validate_auth_invalid_format() {
		let req = create_test_request(Some("12345:deadbeef".to_string()));
		let result = validate_auth(&req, "test_key");
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().error_code, LdkServerErrorCode::AuthError);
	}

	#[test]
	fn test_validate_auth_wrong_key() {
		let timestamp =
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
		let hmac = compute_hmac("wrong_key", timestamp);
		let req = create_test_request(Some(format!("HMAC {timestamp}:{hmac}")));

		let result = validate_auth(&req, "test_api_key");
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().error_code, LdkServerErrorCode::AuthError);
	}

	#[test]
	fn test_validate_auth_expired_timestamp() {
		let timestamp =
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
				- 600;
		let hmac = compute_hmac("test_api_key", timestamp);
		let req = create_test_request(Some(format!("HMAC {timestamp}:{hmac}")));

		let result = validate_auth(&req, "test_api_key");
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().error_code, LdkServerErrorCode::AuthError);
	}
}
