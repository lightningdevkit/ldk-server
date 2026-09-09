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
use hyper::{HeaderMap, Request, Response};
use ldk_node::Node;
use ldk_server_grpc::endpoints::{
	BOLT11_CLAIM_FOR_ID_PATH, BOLT11_FAIL_FOR_ID_PATH, BOLT11_RECEIVE_FOR_HASH_PATH,
	BOLT11_RECEIVE_PATH, BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH,
	BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH, BOLT11_SEND_PATH, BOLT11_SEND_UNDERPAYING_PATH,
	BOLT12_CREATE_PAYER_PROOF_PATH, BOLT12_RECEIVE_PATH, BOLT12_RECEIVE_REFUND_PATH,
	BOLT12_SEND_PATH, BOLT12_SEND_REFUND_PATH, CLOSE_CHANNEL_PATH, CONNECT_PEER_PATH,
	CREATE_MACAROON_PATH, DECODE_INVOICE_PATH, DECODE_OFFER_PATH, DISCONNECT_PEER_PATH,
	EXPORT_PATHFINDING_SCORES_PATH, FORCE_CLOSE_CHANNEL_PATH, GET_BALANCES_PATH, GET_METRICS_PATH,
	GET_NODE_INFO_PATH, GET_PAYMENT_DETAILS_PATH, GET_PERMISSIONS_PATH, GRAPH_GET_CHANNEL_PATH,
	GRAPH_GET_NODE_PATH, GRAPH_LIST_CHANNELS_PATH, GRAPH_LIST_NODES_PATH, LIST_CHANNELS_PATH,
	LIST_FORWARDED_PAYMENTS_PATH, LIST_MACAROONS_PATH, LIST_PAYMENTS_PATH, LIST_PEERS_PATH,
	ONCHAIN_RECEIVE_PATH, ONCHAIN_SEND_PATH, OPEN_CHANNEL_PATH, REVOKE_MACAROON_PATH,
	SIGN_MESSAGE_PATH, SPLICE_IN_PATH, SPLICE_OUT_PATH, SPONTANEOUS_SEND_PATH,
	SUBSCRIBE_EVENTS_PATH, UNIFIED_SEND_PATH, UPDATE_CHANNEL_CONFIG_PATH, VERIFY_SIGNATURE_PATH,
};
use ldk_server_grpc::events::EventEnvelope;
use ldk_server_grpc::grpc::{
	decode_grpc_body, encode_grpc_frame, grpc_error_response, grpc_response, parse_grpc_timeout,
	validate_grpc_request, GrpcBody, GrpcStatus, GRPC_STATUS_DEADLINE_EXCEEDED,
	GRPC_STATUS_FAILED_PRECONDITION, GRPC_STATUS_INTERNAL, GRPC_STATUS_INVALID_ARGUMENT,
	GRPC_STATUS_PERMISSION_DENIED, GRPC_STATUS_UNAUTHENTICATED, GRPC_STATUS_UNAVAILABLE,
	GRPC_STATUS_UNIMPLEMENTED,
};
use prost::Message;
use tokio::sync::{broadcast, mpsc};

use crate::api::bolt11_claim_for_id::handle_bolt11_claim_for_id_request;
use crate::api::bolt11_fail_for_id::handle_bolt11_fail_for_id_request;
use crate::api::bolt11_receive::handle_bolt11_receive_request;
use crate::api::bolt11_receive_for_hash::handle_bolt11_receive_for_hash_request;
use crate::api::bolt11_receive_via_jit_channel::{
	handle_bolt11_receive_variable_amount_via_jit_channel_request,
	handle_bolt11_receive_via_jit_channel_request,
};
use crate::api::bolt11_send::{handle_bolt11_send_request, handle_bolt11_send_underpaying_request};
use crate::api::bolt12_create_payer_proof::handle_bolt12_create_payer_proof_request;
use crate::api::bolt12_receive::handle_bolt12_receive_request;
use crate::api::bolt12_refund::{
	handle_bolt12_receive_refund_request, handle_bolt12_send_refund_request,
};
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
use crate::api::macaroons::{
	handle_create_macaroon_request, handle_get_permissions_request, handle_list_macaroons_request,
	handle_revoke_macaroon_request,
};
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
use crate::macaroons::{method_authorization, MacaroonInfo, MacaroonStore, MethodAuthorization};
use crate::util::metrics::Metrics;

/// gRPC path prefix for the LightningNode service.
const GRPC_SERVICE_PREFIX: &str = "/api.LightningNode/";

// Maximum request body size: 10 MB
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct NodeService {
	context: Arc<Context>,
	macaroon_store: Arc<MacaroonStore>,
	metrics: Option<Arc<Metrics>>,
	metrics_auth_header: Option<String>,
	event_sender: broadcast::Sender<EventEnvelope>,
	shutdown_rx: tokio::sync::watch::Receiver<bool>,
}

impl NodeService {
	pub(crate) fn new(
		node: Arc<Node>, paginated_kv_store: Arc<dyn PaginatedKVStore>,
		macaroon_store: Arc<MacaroonStore>, metrics: Option<Arc<Metrics>>,
		metrics_auth_header: Option<String>, event_sender: broadcast::Sender<EventEnvelope>,
		shutdown_rx: tokio::sync::watch::Receiver<bool>,
	) -> Self {
		let context = Arc::new(Context { node, paginated_kv_store });
		Self { context, macaroon_store, metrics, metrics_auth_header, event_sender, shutdown_rx }
	}
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

		let is_streaming = method == SUBSCRIBE_EVENTS_PATH;
		let macaroon_store = Arc::clone(&self.macaroon_store);
		let event_sender = self.event_sender.clone();
		let shutdown_rx = self.shutdown_rx.clone();
		let (request_parts, request_body) = req.into_parts();
		let future: Self::Future = Box::pin(async move {
			let (issuer, body_bytes) = match read_authorized_request(
				&macaroon_store,
				&method,
				&request_parts.headers,
				request_body,
			)
			.await
			{
				Ok(request) => request,
				Err(status) => return Ok(grpc_error_response(status)),
			};

			match method.as_str() {
				GET_NODE_INFO_PATH => {
					handle_grpc_unary(context, body_bytes, handle_get_node_info_request).await
				},
				GET_BALANCES_PATH => {
					handle_grpc_unary(context, body_bytes, handle_get_balances_request).await
				},
				ONCHAIN_RECEIVE_PATH => {
					handle_grpc_unary(context, body_bytes, handle_onchain_receive_request).await
				},
				ONCHAIN_SEND_PATH => {
					handle_grpc_unary(context, body_bytes, handle_onchain_send_request).await
				},
				BOLT11_RECEIVE_PATH => {
					handle_grpc_unary(context, body_bytes, handle_bolt11_receive_request).await
				},
				BOLT11_RECEIVE_FOR_HASH_PATH => {
					handle_grpc_unary(context, body_bytes, handle_bolt11_receive_for_hash_request)
						.await
				},
				BOLT11_CLAIM_FOR_ID_PATH => {
					handle_grpc_unary(context, body_bytes, handle_bolt11_claim_for_id_request).await
				},
				BOLT11_FAIL_FOR_ID_PATH => {
					handle_grpc_unary(context, body_bytes, handle_bolt11_fail_for_id_request).await
				},
				BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH => {
					handle_grpc_unary(
						context,
						body_bytes,
						handle_bolt11_receive_via_jit_channel_request,
					)
					.await
				},
				BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH => {
					handle_grpc_unary(
						context,
						body_bytes,
						handle_bolt11_receive_variable_amount_via_jit_channel_request,
					)
					.await
				},
				BOLT11_SEND_PATH => {
					handle_grpc_unary(context, body_bytes, handle_bolt11_send_request).await
				},
				BOLT11_SEND_UNDERPAYING_PATH => {
					handle_grpc_unary(context, body_bytes, handle_bolt11_send_underpaying_request)
						.await
				},
				BOLT12_RECEIVE_PATH => {
					handle_grpc_unary(context, body_bytes, handle_bolt12_receive_request).await
				},
				BOLT12_SEND_PATH => {
					handle_grpc_unary(context, body_bytes, handle_bolt12_send_request).await
				},
				BOLT12_SEND_REFUND_PATH => {
					handle_grpc_unary(context, body_bytes, handle_bolt12_send_refund_request).await
				},
				BOLT12_RECEIVE_REFUND_PATH => {
					handle_grpc_unary(context, body_bytes, handle_bolt12_receive_refund_request)
						.await
				},
				BOLT12_CREATE_PAYER_PROOF_PATH => {
					handle_grpc_unary(context, body_bytes, handle_bolt12_create_payer_proof_request)
						.await
				},
				OPEN_CHANNEL_PATH => {
					handle_grpc_unary(context, body_bytes, handle_open_channel).await
				},
				SPLICE_IN_PATH => {
					handle_grpc_unary(context, body_bytes, handle_splice_in_request).await
				},
				SPLICE_OUT_PATH => {
					handle_grpc_unary(context, body_bytes, handle_splice_out_request).await
				},
				CLOSE_CHANNEL_PATH => {
					handle_grpc_unary(context, body_bytes, handle_close_channel_request).await
				},
				FORCE_CLOSE_CHANNEL_PATH => {
					handle_grpc_unary(context, body_bytes, handle_force_close_channel_request).await
				},
				LIST_CHANNELS_PATH => {
					handle_grpc_unary(context, body_bytes, handle_list_channels_request).await
				},
				UPDATE_CHANNEL_CONFIG_PATH => {
					handle_grpc_unary(context, body_bytes, handle_update_channel_config_request)
						.await
				},
				GET_PAYMENT_DETAILS_PATH => {
					handle_grpc_unary(context, body_bytes, handle_get_payment_details_request).await
				},
				LIST_PAYMENTS_PATH => {
					handle_grpc_unary(context, body_bytes, handle_list_payments_request).await
				},
				LIST_FORWARDED_PAYMENTS_PATH => {
					handle_grpc_unary(context, body_bytes, handle_list_forwarded_payments_request)
						.await
				},
				CONNECT_PEER_PATH => {
					handle_grpc_unary(context, body_bytes, handle_connect_peer).await
				},
				DISCONNECT_PEER_PATH => {
					handle_grpc_unary(context, body_bytes, handle_disconnect_peer).await
				},
				LIST_PEERS_PATH => {
					handle_grpc_unary(context, body_bytes, handle_list_peers_request).await
				},
				SPONTANEOUS_SEND_PATH => {
					handle_grpc_unary(context, body_bytes, handle_spontaneous_send_request).await
				},
				UNIFIED_SEND_PATH => {
					handle_grpc_unary(context, body_bytes, handle_unified_send_request).await
				},
				SIGN_MESSAGE_PATH => {
					handle_grpc_unary(context, body_bytes, handle_sign_message_request).await
				},
				VERIFY_SIGNATURE_PATH => {
					handle_grpc_unary(context, body_bytes, handle_verify_signature_request).await
				},
				EXPORT_PATHFINDING_SCORES_PATH => {
					handle_grpc_unary(context, body_bytes, handle_export_pathfinding_scores_request)
						.await
				},
				GRAPH_LIST_CHANNELS_PATH => {
					handle_grpc_unary(context, body_bytes, handle_graph_list_channels_request).await
				},
				GRAPH_GET_CHANNEL_PATH => {
					handle_grpc_unary(context, body_bytes, handle_graph_get_channel_request).await
				},
				GRAPH_LIST_NODES_PATH => {
					handle_grpc_unary(context, body_bytes, handle_graph_list_nodes_request).await
				},
				GRAPH_GET_NODE_PATH => {
					handle_grpc_unary(context, body_bytes, handle_graph_get_node_request).await
				},
				DECODE_INVOICE_PATH => {
					handle_grpc_unary(context, body_bytes, handle_decode_invoice_request).await
				},
				DECODE_OFFER_PATH => {
					handle_grpc_unary(context, body_bytes, handle_decode_offer_request).await
				},
				SUBSCRIBE_EVENTS_PATH => {
					// Authorization applies when the subscription starts; revocation does not close it.
					let mut shutdown_rx = shutdown_rx;
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
								}
							}
						}
					});
					Ok(grpc_response(GrpcBody::Stream { rx: mpsc_rx, done: false }))
				},
				CREATE_MACAROON_PATH => {
					let store = Arc::clone(&macaroon_store);
					handle_grpc_unary(context, body_bytes, move |_context, request| {
						handle_create_macaroon_request(store, issuer, request)
					})
					.await
				},
				LIST_MACAROONS_PATH => {
					let store = Arc::clone(&macaroon_store);
					handle_grpc_unary(context, body_bytes, move |_context, request| {
						handle_list_macaroons_request(store, request)
					})
					.await
				},
				REVOKE_MACAROON_PATH => {
					let store = Arc::clone(&macaroon_store);
					handle_grpc_unary(context, body_bytes, move |_context, request| {
						handle_revoke_macaroon_request(store, issuer, request)
					})
					.await
				},
				GET_PERMISSIONS_PATH => {
					handle_grpc_unary(context, body_bytes, move |_context, request| {
						handle_get_permissions_request(issuer, request)
					})
					.await
				},
				_ => {
					let status = GrpcStatus::new(
						GRPC_STATUS_UNIMPLEMENTED,
						format!("Unknown method: {method}"),
					);
					Ok(grpc_error_response(status))
				},
			}
		});

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
	F: FnOnce(Arc<Context>, T) -> Fut + Send,
>(
	context: Arc<Context>, body_bytes: bytes::Bytes, handler: F,
) -> Result<Response<GrpcBody>, hyper::Error> {
	// Decode gRPC framing then protobuf
	let req_msg = decode_grpc_body(&body_bytes)
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

fn request_content_length(headers: &HeaderMap) -> Result<Option<u64>, GrpcStatus> {
	let Some(content_length) = headers.get("content-length") else {
		return Ok(None);
	};
	let len = content_length.to_str().ok().and_then(|value| value.parse::<u64>().ok()).ok_or_else(
		|| GrpcStatus::new(GRPC_STATUS_INVALID_ARGUMENT, "Invalid content-length header"),
	)?;
	if len > MAX_BODY_SIZE as u64 {
		return Err(GrpcStatus::new(
			GRPC_STATUS_INVALID_ARGUMENT,
			"Request body too large or failed to read",
		));
	}
	Ok(Some(len))
}

fn validate_request_body_len(
	content_length: Option<u64>, actual_len: usize,
) -> Result<(), GrpcStatus> {
	if let Some(expected_len) = content_length {
		if expected_len != actual_len as u64 {
			return Err(GrpcStatus::new(
				GRPC_STATUS_INVALID_ARGUMENT,
				"Request body length does not match content-length",
			));
		}
	}
	Ok(())
}

async fn read_authorized_request<B>(
	store: &MacaroonStore, method: &str, headers: &HeaderMap, body: B,
) -> Result<(Arc<MacaroonInfo>, bytes::Bytes), GrpcStatus>
where
	B: hyper::body::Body<Data = bytes::Bytes>,
	B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
	let auth_header = headers.get("macaroon").and_then(|value| value.to_str().ok());
	let request =
		store.authenticate_request(method, auth_header).map_err(ldk_error_to_grpc_status)?;
	match method_authorization(method) {
		MethodAuthorization::Permission(permission) if !request.info.allows(permission) => {
			return Err(GrpcStatus::new(
				GRPC_STATUS_PERMISSION_DENIED,
				format!("macaroon requires permission: {permission}"),
			));
		},
		MethodAuthorization::Unknown => {
			return Err(GrpcStatus::new(
				GRPC_STATUS_UNIMPLEMENTED,
				format!("Unknown method: {method}"),
			));
		},
		_ => {},
	}
	let content_length = request_content_length(headers)?;
	let limited_body = Limited::new(body, MAX_BODY_SIZE);
	let bytes = match limited_body.collect().await {
		Ok(collected) => collected.to_bytes(),
		Err(_) => {
			return Err(GrpcStatus::new(
				GRPC_STATUS_INVALID_ARGUMENT,
				"Request body too large or failed to read",
			));
		},
	};
	validate_request_body_len(content_length, bytes.len())?;
	let info = store.finish_request(request, method, &bytes).map_err(ldk_error_to_grpc_status)?;
	Ok((info, bytes))
}

/// Map an `LdkServerError` to a `GrpcStatus`.
pub(crate) fn ldk_error_to_grpc_status(e: LdkServerError) -> GrpcStatus {
	let code = match e.error_code {
		LdkServerErrorCode::InvalidRequestError => GRPC_STATUS_INVALID_ARGUMENT,
		LdkServerErrorCode::AuthError => GRPC_STATUS_UNAUTHENTICATED,
		LdkServerErrorCode::AuthorizationError => GRPC_STATUS_PERMISSION_DENIED,
		LdkServerErrorCode::LightningError => GRPC_STATUS_FAILED_PRECONDITION,
		LdkServerErrorCode::InternalServerError => GRPC_STATUS_INTERNAL,
	};
	GrpcStatus { code, message: e.message }
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::macaroons::test_util::{admin_token, bind_request, test_store};

	struct UnreadBody;
	impl hyper::body::Body for UnreadBody {
		type Data = bytes::Bytes;
		type Error = std::convert::Infallible;
		fn poll_frame(
			self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>,
		) -> std::task::Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
			panic!("Rejected request body must not be read");
		}
	}

	#[tokio::test]
	async fn macaroon_request_clock_skew() {
		use ldk_server_grpc::grpc::GRPC_STATUS_OK;

		let (_directory, store) = test_store("http-clock-skew");
		let token = admin_token(&store);
		let bytes = [0; 5]; // Empty protobuf message with its gRPC frame header.

		// Exact 60-second boundaries are tested with a fixed clock in policy tests.
		for (offset, expected) in [
			(0i64, GRPC_STATUS_OK),
			(-30, GRPC_STATUS_OK),
			(30, GRPC_STATUS_OK),
			(-120, GRPC_STATUS_UNAUTHENTICATED),
			(120, GRPC_STATUS_UNAUTHENTICATED),
		] {
			let now = std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.unwrap()
				.as_secs();
			let bound = bind_request(
				&token,
				GET_NODE_INFO_PATH,
				&bytes,
				now.checked_add_signed(offset).unwrap(),
			);
			let mut headers = HeaderMap::new();
			headers.insert("macaroon", bound.parse().unwrap());
			let body = http_body_util::Full::new(bytes::Bytes::copy_from_slice(&bytes));
			let status =
				match read_authorized_request(&store, GET_NODE_INFO_PATH, &headers, body).await {
					Ok((_, received)) => {
						assert_eq!(received.as_ref(), &bytes);
						GRPC_STATUS_OK
					},
					Err(error) => error.code,
				};
			assert_eq!(status, expected, "timestamp offset: {offset}");
		}
	}

	#[tokio::test]
	async fn rejected_requests_do_not_poll_the_body() {
		let (_directory, store) = test_store("http-admission");
		let token = admin_token(&store);
		let admin = store.authenticate(CREATE_MACAROON_PATH, Some(&token)).unwrap();
		let reader = store.create_root("reader", vec!["node:read".into()], &admin).unwrap();
		let timestamp =
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
		let denied = bind_request(&reader.token, ONCHAIN_SEND_PATH, b"", timestamp);
		let unknown = bind_request(&token, "UnmappedMethod", b"", timestamp);
		let stale = bind_request(&token, GET_NODE_INFO_PATH, b"", timestamp - 61);
		let wrong_method = bind_request(&token, GET_BALANCES_PATH, b"", timestamp);
		for (credential, method, expected) in [
			(None, GET_NODE_INFO_PATH, GRPC_STATUS_UNAUTHENTICATED),
			(None, GET_PERMISSIONS_PATH, GRPC_STATUS_UNAUTHENTICATED),
			(Some("invalid"), GET_NODE_INFO_PATH, GRPC_STATUS_UNAUTHENTICATED),
			(Some(reader.token.as_str()), GET_NODE_INFO_PATH, GRPC_STATUS_UNAUTHENTICATED),
			(Some(denied.as_str()), ONCHAIN_SEND_PATH, GRPC_STATUS_PERMISSION_DENIED),
			(Some(unknown.as_str()), "UnmappedMethod", GRPC_STATUS_UNIMPLEMENTED),
			(Some(stale.as_str()), GET_NODE_INFO_PATH, GRPC_STATUS_UNAUTHENTICATED),
			(Some(wrong_method.as_str()), GET_NODE_INFO_PATH, GRPC_STATUS_UNAUTHENTICATED),
		] {
			let mut headers = HeaderMap::new();
			if let Some(token) = credential {
				headers.insert("macaroon", token.parse().unwrap());
			}
			let error =
				read_authorized_request(&store, method, &headers, UnreadBody).await.unwrap_err();
			assert_eq!(error.code, expected);
		}
		let mut headers = HeaderMap::new();
		let bound = bind_request(&reader.token, GET_NODE_INFO_PATH, b"request", timestamp);
		headers.insert("macaroon", bound.parse().unwrap());
		let body = http_body_util::Full::new(bytes::Bytes::from_static(b"request"));
		let (_, bytes) =
			read_authorized_request(&store, GET_NODE_INFO_PATH, &headers, body).await.unwrap();
		assert_eq!(bytes.as_ref(), b"request");
		let changed_body = http_body_util::Full::new(bytes::Bytes::from_static(b"changed"));
		assert_eq!(
			read_authorized_request(&store, GET_NODE_INFO_PATH, &headers, changed_body)
				.await
				.unwrap_err()
				.code,
			GRPC_STATUS_UNAUTHENTICATED
		);
		// Authorized requests still have both declared and actual body-size limits.
		headers.insert("content-length", (MAX_BODY_SIZE + 1).to_string().parse().unwrap());
		assert_eq!(
			read_authorized_request(&store, GET_NODE_INFO_PATH, &headers, UnreadBody)
				.await
				.unwrap_err()
				.code,
			GRPC_STATUS_INVALID_ARGUMENT
		);
		headers.remove("content-length");
		let oversized = http_body_util::Full::new(bytes::Bytes::from(vec![0; MAX_BODY_SIZE + 1]));
		assert_eq!(
			read_authorized_request(&store, GET_NODE_INFO_PATH, &headers, oversized)
				.await
				.unwrap_err()
				.code,
			GRPC_STATUS_INVALID_ARGUMENT
		);
	}

	#[tokio::test]
	async fn malformed_macaroon_headers_are_rejected_before_reading_the_body() {
		use hyper::header::HeaderValue;
		use ldk_server_macaroons::MAX_MACAROON_BYTES;

		let (_directory, store) = test_store("http-admission");
		let token = admin_token(&store);
		let timestamp =
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
		let body = b"\x00\x00\x00\x00\x00";
		let bound = bind_request(&token, GET_NODE_INFO_PATH, body, timestamp);
		let mut headers = HeaderMap::new();
		headers.insert("macaroon", bound.parse().unwrap());
		let (_, received) = read_authorized_request(
			&store,
			GET_NODE_INFO_PATH,
			&headers,
			http_body_util::Full::new(bytes::Bytes::from_static(body)),
		)
		.await
		.unwrap();
		assert_eq!(received.as_ref(), body);

		for (case, header) in [
			("missing", None),
			("empty", Some(Vec::new())),
			("non-ASCII", Some(vec![0xff])),
			("non-hex", Some(b"zz".to_vec())),
			("odd hex length", Some(bound.as_bytes()[..bound.len() - 1].to_vec())),
			("truncated token", Some(bound.as_bytes()[..bound.len() - 2].to_vec())),
			("trailing bytes", Some(format!("{bound}00").into_bytes())),
			("extra prefix", Some(format!("Bearer {bound}").into_bytes())),
			("unbound token", Some(token.into_bytes())),
			("oversized token", Some("00".repeat(MAX_MACAROON_BYTES + 1).into_bytes())),
		] {
			let mut headers = HeaderMap::new();
			if let Some(header) = header {
				headers.insert("macaroon", HeaderValue::from_bytes(&header).unwrap());
			}
			let error = read_authorized_request(&store, GET_NODE_INFO_PATH, &headers, UnreadBody)
				.await
				.unwrap_err();
			assert_eq!(error.code, GRPC_STATUS_UNAUTHENTICATED, "header case: {case}");
		}
	}

	#[tokio::test]
	async fn policy_expiry_during_body_read_is_rejected() {
		use std::time::{Duration, SystemTime, UNIX_EPOCH};

		let (_directory, store) = test_store("http-expiry");
		let token = admin_token(&store);
		let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
		let expiry = timestamp + 3;
		let token =
			ldk_server_macaroons::derive_macaroon(&token, &[format!("time-before = {expiry}")])
				.unwrap();
		let bytes = bytes::Bytes::from_static(&[0; 5]);
		let bound = bind_request(&token, GET_NODE_INFO_PATH, &bytes, timestamp);
		let mut headers = HeaderMap::new();
		headers.insert("macaroon", bound.parse().unwrap());
		// The same credential and body must pass before the policy expires.
		read_authorized_request(
			&store,
			GET_NODE_INFO_PATH,
			&headers,
			http_body_util::Full::new(bytes.clone()),
		)
		.await
		.unwrap();

		let body_started = std::cell::Cell::new(false);
		let body = http_body_util::StreamBody::new(futures_util::stream::once(async {
			body_started.set(true);
			while SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() < expiry {
				tokio::time::sleep(Duration::from_millis(50)).await;
			}
			Ok::<_, std::convert::Infallible>(hyper::body::Frame::data(bytes))
		}));
		let error = tokio::time::timeout(
			Duration::from_secs(10),
			read_authorized_request(&store, GET_NODE_INFO_PATH, &headers, body),
		)
		.await
		.unwrap()
		.unwrap_err();
		assert!(body_started.get(), "Request must pass authentication before its body is read");
		assert_eq!(error.code, GRPC_STATUS_PERMISSION_DENIED);
		assert_eq!(error.message, "Macaroon expired");
	}

	#[test]
	fn test_request_content_length_missing() {
		let headers = HeaderMap::new();
		assert_eq!(request_content_length(&headers).unwrap(), None);
	}

	#[test]
	fn test_request_content_length_parses_value() {
		let mut headers = HeaderMap::new();
		headers.insert("content-length", "42".parse().unwrap());

		assert_eq!(request_content_length(&headers).unwrap(), Some(42));
	}

	#[test]
	fn test_request_content_length_rejects_invalid_value() {
		let mut headers = HeaderMap::new();
		headers.insert("content-length", "not-a-number".parse().unwrap());

		let err = request_content_length(&headers).unwrap_err();
		assert_eq!(err.code, GRPC_STATUS_INVALID_ARGUMENT);
		assert_eq!(err.message, "Invalid content-length header");
	}

	#[test]
	fn test_request_content_length_rejects_oversized_value() {
		let mut headers = HeaderMap::new();
		headers.insert("content-length", (MAX_BODY_SIZE as u64 + 1).to_string().parse().unwrap());

		let err = request_content_length(&headers).unwrap_err();
		assert_eq!(err.code, GRPC_STATUS_INVALID_ARGUMENT);
		assert_eq!(err.message, "Request body too large or failed to read");
	}

	#[test]
	fn test_validate_request_body_len_allows_matching_length() {
		assert!(validate_request_body_len(Some(5), 5).is_ok());
	}

	#[test]
	fn test_validate_request_body_len_allows_missing_length() {
		assert!(validate_request_body_len(None, 5).is_ok());
	}

	#[test]
	fn test_validate_request_body_len_rejects_mismatch() {
		let err = validate_request_body_len(Some(6), 5).unwrap_err();
		assert_eq!(err.code, GRPC_STATUS_INVALID_ARGUMENT);
		assert_eq!(err.message, "Request body length does not match content-length");
	}
}
