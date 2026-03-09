// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::service::Service;
use hyper::{Request, Response, StatusCode};
use ldk_node::Node;
use ldk_server_protos::api::{
	CreateApiKeyRequest, CreateApiKeyResponse, GetPermissionsRequest, GetPermissionsResponse,
};
use ldk_server_protos::endpoints::{
	BOLT11_RECEIVE_PATH, BOLT11_SEND_PATH, BOLT12_RECEIVE_PATH, BOLT12_SEND_PATH,
	CLOSE_CHANNEL_PATH, CONNECT_PEER_PATH, CREATE_API_KEY_PATH, DISCONNECT_PEER_PATH,
	EXPORT_PATHFINDING_SCORES_PATH, FORCE_CLOSE_CHANNEL_PATH, GET_BALANCES_PATH,
	GET_NODE_INFO_PATH, GET_PAYMENT_DETAILS_PATH, GET_PERMISSIONS_PATH, GRAPH_GET_CHANNEL_PATH,
	GRAPH_GET_NODE_PATH, GRAPH_LIST_CHANNELS_PATH, GRAPH_LIST_NODES_PATH, LIST_CHANNELS_PATH,
	LIST_FORWARDED_PAYMENTS_PATH, LIST_PAYMENTS_PATH, ONCHAIN_RECEIVE_PATH, ONCHAIN_SEND_PATH,
	OPEN_CHANNEL_PATH, SIGN_MESSAGE_PATH, SPLICE_IN_PATH, SPLICE_OUT_PATH, SPONTANEOUS_SEND_PATH,
	UPDATE_CHANNEL_CONFIG_PATH, VERIFY_SIGNATURE_PATH,
};
use prost::Message;

use crate::api::bolt11_receive::handle_bolt11_receive_request;
use crate::api::bolt11_send::handle_bolt11_send_request;
use crate::api::bolt12_receive::handle_bolt12_receive_request;
use crate::api::bolt12_send::handle_bolt12_send_request;
use crate::api::close_channel::{handle_close_channel_request, handle_force_close_channel_request};
use crate::api::connect_peer::handle_connect_peer;
use crate::api::disconnect_peer::handle_disconnect_peer;
use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::{AuthError, InvalidRequestError};
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
use crate::api::onchain_receive::handle_onchain_receive_request;
use crate::api::onchain_send::handle_onchain_send_request;
use crate::api::open_channel::handle_open_channel;
use crate::api::sign_message::handle_sign_message_request;
use crate::api::splice_channel::{handle_splice_in_request, handle_splice_out_request};
use crate::api::spontaneous_send::handle_spontaneous_send_request;
use crate::api::update_channel_config::handle_update_channel_config_request;
use crate::api::verify_signature::handle_verify_signature_request;
use crate::api_keys::ApiKeyStore;
use crate::io::persist::paginated_kv_store::PaginatedKVStore;
use crate::util::proto_adapter::to_error_response;

// Maximum request body size: 10 MB
// This prevents memory exhaustion from large requests
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

#[derive(Clone)]
pub struct NodeService {
	node: Arc<Node>,
	paginated_kv_store: Arc<dyn PaginatedKVStore>,
	api_key_store: Arc<RwLock<ApiKeyStore>>,
}

impl NodeService {
	pub(crate) fn new(
		node: Arc<Node>, paginated_kv_store: Arc<dyn PaginatedKVStore>,
		api_key_store: Arc<RwLock<ApiKeyStore>>,
	) -> Self {
		Self { node, paginated_kv_store, api_key_store }
	}
}

// Maximum allowed time difference between client timestamp and server time (1 minute)
pub(crate) const AUTH_TIMESTAMP_TOLERANCE_SECS: u64 = 60;

#[derive(Debug, Clone)]
pub(crate) struct AuthParams {
	key_id: String,
	timestamp: u64,
	hmac_hex: String,
}

/// Extracts authentication parameters from request headers.
/// Returns (key_id, timestamp, hmac_hex) if valid format, or error.
fn extract_auth_params<B>(req: &Request<B>) -> Result<AuthParams, LdkServerError> {
	let auth_header = req
		.headers()
		.get("X-Auth")
		.and_then(|v| v.to_str().ok())
		.ok_or_else(|| LdkServerError::new(AuthError, "Missing X-Auth header"))?;

	// Format: "HMAC <key_id>:<timestamp>:<hmac_hex>"
	let auth_data = auth_header
		.strip_prefix("HMAC ")
		.ok_or_else(|| LdkServerError::new(AuthError, "Invalid X-Auth header format"))?;

	let parts: Vec<&str> = auth_data.splitn(3, ':').collect();
	if parts.len() != 3 {
		return Err(LdkServerError::new(AuthError, "Invalid X-Auth header format"));
	}

	let key_id = parts[0];
	let timestamp_str = parts[1];
	let hmac_hex = parts[2];

	// Validate key_id is 16 hex chars
	if key_id.len() != 16 || !key_id.chars().all(|c| c.is_ascii_hexdigit()) {
		return Err(LdkServerError::new(AuthError, "Invalid key_id in X-Auth header"));
	}

	let timestamp = timestamp_str
		.parse::<u64>()
		.map_err(|_| LdkServerError::new(AuthError, "Invalid timestamp in X-Auth header"))?;

	// validate hmac_hex is valid hex
	if hmac_hex.len() != 64 || !hmac_hex.chars().all(|c| c.is_ascii_hexdigit()) {
		return Err(LdkServerError::new(AuthError, "Invalid HMAC in X-Auth header"));
	}

	Ok(AuthParams { key_id: key_id.to_string(), timestamp, hmac_hex: hmac_hex.to_string() })
}

fn handle_get_permissions_request(
	_context: Context, _request: GetPermissionsRequest, endpoints: HashSet<String>,
) -> Result<GetPermissionsResponse, LdkServerError> {
	// Sort for deterministic response ordering since endpoints are stored in a HashSet.
	let mut endpoints: Vec<String> = endpoints.into_iter().collect();
	endpoints.sort();
	Ok(GetPermissionsResponse { endpoints })
}

fn handle_create_api_key_request(
	_context: Context, request: CreateApiKeyRequest, endpoints: HashSet<String>,
	api_key_store: Arc<RwLock<ApiKeyStore>>,
) -> Result<CreateApiKeyResponse, LdkServerError> {
	// Only admin keys (with "*" wildcard) can create new keys
	if !endpoints.contains("*") {
		return Err(LdkServerError::new(AuthError, "Only admin keys can create new API keys"));
	}

	let mut store = api_key_store.write().map_err(|_| {
		LdkServerError::new(InvalidRequestError, "Failed to acquire API key store lock")
	})?;

	let api_key = store.create_key(&request.name, request.endpoints)?;
	Ok(CreateApiKeyResponse { api_key })
}

pub(crate) struct Context {
	pub(crate) node: Arc<Node>,
	pub(crate) paginated_kv_store: Arc<dyn PaginatedKVStore>,
}

macro_rules! route {
	($context:expr, $req:expr, $auth_params:expr, $api_key_store:expr, $endpoint:expr, $handler:expr) => {
		Box::pin(handle_request($context, $req, $auth_params, $api_key_store, $endpoint, $handler))
	};
}

impl Service<Request<Incoming>> for NodeService {
	type Response = Response<Full<Bytes>>;
	type Error = hyper::Error;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

	fn call(&self, req: Request<Incoming>) -> Self::Future {
		// Extract auth params from headers (validation happens after body is read)
		let auth_params = match extract_auth_params(&req) {
			Ok(params) => params,
			Err(e) => {
				let (error_response, status_code) = to_error_response(e);
				return Box::pin(async move {
					Ok(Response::builder()
						.status(status_code)
						.body(Full::new(Bytes::from(error_response.encode_to_vec())))
						// unwrap safety: body only errors when previous chained calls failed.
						.unwrap())
				});
			},
		};

		let context = Context {
			node: Arc::clone(&self.node),
			paginated_kv_store: Arc::clone(&self.paginated_kv_store),
		};
		let api_key_store = Arc::clone(&self.api_key_store);

		// Exclude '/' from path pattern matching.
		match &req.uri().path()[1..] {
			GET_NODE_INFO_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					GET_NODE_INFO_PATH,
					handle_get_node_info_request
				)
			},
			GET_BALANCES_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					GET_BALANCES_PATH,
					handle_get_balances_request
				)
			},
			ONCHAIN_RECEIVE_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					ONCHAIN_RECEIVE_PATH,
					handle_onchain_receive_request
				)
			},
			ONCHAIN_SEND_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					ONCHAIN_SEND_PATH,
					handle_onchain_send_request
				)
			},
			BOLT11_RECEIVE_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					BOLT11_RECEIVE_PATH,
					handle_bolt11_receive_request
				)
			},
			BOLT11_SEND_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					BOLT11_SEND_PATH,
					handle_bolt11_send_request
				)
			},
			BOLT12_RECEIVE_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					BOLT12_RECEIVE_PATH,
					handle_bolt12_receive_request
				)
			},
			BOLT12_SEND_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					BOLT12_SEND_PATH,
					handle_bolt12_send_request
				)
			},
			OPEN_CHANNEL_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					OPEN_CHANNEL_PATH,
					handle_open_channel
				)
			},
			SPLICE_IN_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					SPLICE_IN_PATH,
					handle_splice_in_request
				)
			},
			SPLICE_OUT_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					SPLICE_OUT_PATH,
					handle_splice_out_request
				)
			},
			CLOSE_CHANNEL_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					CLOSE_CHANNEL_PATH,
					handle_close_channel_request
				)
			},
			FORCE_CLOSE_CHANNEL_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					FORCE_CLOSE_CHANNEL_PATH,
					handle_force_close_channel_request
				)
			},
			LIST_CHANNELS_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					LIST_CHANNELS_PATH,
					handle_list_channels_request
				)
			},
			UPDATE_CHANNEL_CONFIG_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					UPDATE_CHANNEL_CONFIG_PATH,
					handle_update_channel_config_request
				)
			},
			GET_PAYMENT_DETAILS_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					GET_PAYMENT_DETAILS_PATH,
					handle_get_payment_details_request
				)
			},
			LIST_PAYMENTS_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					LIST_PAYMENTS_PATH,
					handle_list_payments_request
				)
			},
			LIST_FORWARDED_PAYMENTS_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					LIST_FORWARDED_PAYMENTS_PATH,
					handle_list_forwarded_payments_request
				)
			},
			CONNECT_PEER_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					CONNECT_PEER_PATH,
					handle_connect_peer
				)
			},
			DISCONNECT_PEER_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					DISCONNECT_PEER_PATH,
					handle_disconnect_peer
				)
			},
			SPONTANEOUS_SEND_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					SPONTANEOUS_SEND_PATH,
					handle_spontaneous_send_request
				)
			},
			SIGN_MESSAGE_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					SIGN_MESSAGE_PATH,
					handle_sign_message_request
				)
			},
			VERIFY_SIGNATURE_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					VERIFY_SIGNATURE_PATH,
					handle_verify_signature_request
				)
			},
			EXPORT_PATHFINDING_SCORES_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					EXPORT_PATHFINDING_SCORES_PATH,
					handle_export_pathfinding_scores_request
				)
			},
			GET_PERMISSIONS_PATH => {
				Box::pin(handle_permissions_request(context, req, auth_params, api_key_store))
			},
			CREATE_API_KEY_PATH => {
				Box::pin(handle_create_key_request(context, req, auth_params, api_key_store))
			},
			GRAPH_LIST_CHANNELS_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					GRAPH_LIST_CHANNELS_PATH,
					handle_graph_list_channels_request
				)
			},
			GRAPH_GET_CHANNEL_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					GRAPH_GET_CHANNEL_PATH,
					handle_graph_get_channel_request
				)
			},
			GRAPH_LIST_NODES_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					GRAPH_LIST_NODES_PATH,
					handle_graph_list_nodes_request
				)
			},
			GRAPH_GET_NODE_PATH => {
				route!(
					context,
					req,
					auth_params,
					api_key_store,
					GRAPH_GET_NODE_PATH,
					handle_graph_get_node_request
				)
			},
			path => {
				let error = format!("Unknown request: {}", path).into_bytes();
				Box::pin(async {
					Ok(Response::builder()
						.status(StatusCode::BAD_REQUEST)
						.body(Full::new(Bytes::from(error)))
						// unwrap safety: body only errors when previous chained calls failed.
						.unwrap())
				})
			},
		}
	}
}

async fn handle_request<
	T: Message + Default,
	R: Message,
	F: Fn(Context, T) -> Result<R, LdkServerError>,
>(
	context: Context, request: Request<Incoming>, auth_params: AuthParams,
	api_key_store: Arc<RwLock<ApiKeyStore>>, endpoint: &str, handler: F,
) -> Result<<NodeService as Service<Request<Incoming>>>::Response, hyper::Error> {
	// Limit the size of the request body to prevent abuse
	let limited_body = Limited::new(request.into_body(), MAX_BODY_SIZE);
	let bytes = match limited_body.collect().await {
		Ok(collected) => collected.to_bytes(),
		Err(_) => {
			let (error_response, status_code) = to_error_response(LdkServerError::new(
				InvalidRequestError,
				"Request body too large or failed to read.",
			));
			return Ok(Response::builder()
				.status(status_code)
				.body(Full::new(Bytes::from(error_response.encode_to_vec())))
				// unwrap safety: body only errors when previous chained calls failed.
				.unwrap());
		},
	};

	// Validate HMAC authentication and endpoint authorization
	let auth_result = {
		let store = api_key_store.read().map_err(|_| {
			// hyper::Error can't be constructed directly; return an auth error response instead
		});
		match store {
			Ok(store) => store.validate_and_authorize(
				endpoint,
				&auth_params.key_id,
				auth_params.timestamp,
				&auth_params.hmac_hex,
				&bytes,
			),
			Err(_) => Err(LdkServerError::new(AuthError, "Failed to acquire API key store lock")),
		}
	};

	if let Err(e) = auth_result {
		let (error_response, status_code) = to_error_response(e);
		return Ok(Response::builder()
			.status(status_code)
			.body(Full::new(Bytes::from(error_response.encode_to_vec())))
			// unwrap safety: body only errors when previous chained calls failed.
			.unwrap());
	}

	match T::decode(bytes) {
		Ok(request) => match handler(context, request) {
			Ok(response) => Ok(Response::builder()
				.body(Full::new(Bytes::from(response.encode_to_vec())))
				// unwrap safety: body only errors when previous chained calls failed.
				.unwrap()),
			Err(e) => {
				let (error_response, status_code) = to_error_response(e);
				Ok(Response::builder()
					.status(status_code)
					.body(Full::new(Bytes::from(error_response.encode_to_vec())))
					// unwrap safety: body only errors when previous chained calls failed.
					.unwrap())
			},
		},
		Err(_) => {
			let (error_response, status_code) =
				to_error_response(LdkServerError::new(InvalidRequestError, "Malformed request."));
			Ok(Response::builder()
				.status(status_code)
				.body(Full::new(Bytes::from(error_response.encode_to_vec())))
				// unwrap safety: body only errors when previous chained calls failed.
				.unwrap())
		},
	}
}

async fn handle_permissions_request(
	context: Context, request: Request<Incoming>, auth_params: AuthParams,
	api_key_store: Arc<RwLock<ApiKeyStore>>,
) -> Result<<NodeService as Service<Request<Incoming>>>::Response, hyper::Error> {
	let limited_body = Limited::new(request.into_body(), MAX_BODY_SIZE);
	let bytes = match limited_body.collect().await {
		Ok(collected) => collected.to_bytes(),
		Err(_) => {
			let (error_response, status_code) = to_error_response(LdkServerError::new(
				InvalidRequestError,
				"Request body too large or failed to read.",
			));
			return Ok(Response::builder()
				.status(status_code)
				.body(Full::new(Bytes::from(error_response.encode_to_vec())))
				.unwrap());
		},
	};

	let endpoints = {
		let store = match api_key_store.read() {
			Ok(s) => s,
			Err(_) => {
				let (error_response, status_code) = to_error_response(LdkServerError::new(
					AuthError,
					"Failed to acquire API key store lock",
				));
				return Ok(Response::builder()
					.status(status_code)
					.body(Full::new(Bytes::from(error_response.encode_to_vec())))
					.unwrap());
			},
		};
		match store.validate_and_authorize(
			GET_PERMISSIONS_PATH,
			&auth_params.key_id,
			auth_params.timestamp,
			&auth_params.hmac_hex,
			&bytes,
		) {
			Ok(endpoints) => endpoints,
			Err(e) => {
				let (error_response, status_code) = to_error_response(e);
				return Ok(Response::builder()
					.status(status_code)
					.body(Full::new(Bytes::from(error_response.encode_to_vec())))
					.unwrap());
			},
		}
	};

	match GetPermissionsRequest::decode(bytes) {
		Ok(req) => match handle_get_permissions_request(context, req, endpoints) {
			Ok(response) => Ok(Response::builder()
				.body(Full::new(Bytes::from(response.encode_to_vec())))
				.unwrap()),
			Err(e) => {
				let (error_response, status_code) = to_error_response(e);
				Ok(Response::builder()
					.status(status_code)
					.body(Full::new(Bytes::from(error_response.encode_to_vec())))
					.unwrap())
			},
		},
		Err(_) => {
			let (error_response, status_code) =
				to_error_response(LdkServerError::new(InvalidRequestError, "Malformed request."));
			Ok(Response::builder()
				.status(status_code)
				.body(Full::new(Bytes::from(error_response.encode_to_vec())))
				.unwrap())
		},
	}
}

async fn handle_create_key_request(
	context: Context, request: Request<Incoming>, auth_params: AuthParams,
	api_key_store: Arc<RwLock<ApiKeyStore>>,
) -> Result<<NodeService as Service<Request<Incoming>>>::Response, hyper::Error> {
	let limited_body = Limited::new(request.into_body(), MAX_BODY_SIZE);
	let bytes = match limited_body.collect().await {
		Ok(collected) => collected.to_bytes(),
		Err(_) => {
			let (error_response, status_code) = to_error_response(LdkServerError::new(
				InvalidRequestError,
				"Request body too large or failed to read.",
			));
			return Ok(Response::builder()
				.status(status_code)
				.body(Full::new(Bytes::from(error_response.encode_to_vec())))
				.unwrap());
		},
	};

	let endpoints = {
		let store = match api_key_store.read() {
			Ok(s) => s,
			Err(_) => {
				let (error_response, status_code) = to_error_response(LdkServerError::new(
					AuthError,
					"Failed to acquire API key store lock",
				));
				return Ok(Response::builder()
					.status(status_code)
					.body(Full::new(Bytes::from(error_response.encode_to_vec())))
					.unwrap());
			},
		};
		match store.validate_and_authorize(
			CREATE_API_KEY_PATH,
			&auth_params.key_id,
			auth_params.timestamp,
			&auth_params.hmac_hex,
			&bytes,
		) {
			Ok(endpoints) => endpoints,
			Err(e) => {
				let (error_response, status_code) = to_error_response(e);
				return Ok(Response::builder()
					.status(status_code)
					.body(Full::new(Bytes::from(error_response.encode_to_vec())))
					.unwrap());
			},
		}
	};

	match CreateApiKeyRequest::decode(bytes) {
		Ok(req) => {
			match handle_create_api_key_request(context, req, endpoints, Arc::clone(&api_key_store))
			{
				Ok(response) => Ok(Response::builder()
					.body(Full::new(Bytes::from(response.encode_to_vec())))
					.unwrap()),
				Err(e) => {
					let (error_response, status_code) = to_error_response(e);
					Ok(Response::builder()
						.status(status_code)
						.body(Full::new(Bytes::from(error_response.encode_to_vec())))
						.unwrap())
				},
			}
		},
		Err(_) => {
			let (error_response, status_code) =
				to_error_response(LdkServerError::new(InvalidRequestError, "Malformed request."));
			Ok(Response::builder()
				.status(status_code)
				.body(Full::new(Bytes::from(error_response.encode_to_vec())))
				.unwrap())
		},
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn create_test_request(auth_header: Option<String>) -> Request<()> {
		let mut builder = Request::builder();
		if let Some(header) = auth_header {
			builder = builder.header("X-Auth", header);
		}
		builder.body(()).unwrap()
	}

	#[test]
	fn test_extract_auth_params_success() {
		let timestamp =
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
		let hmac = "8f5a33c2c68fb253899a588308fd13dcaf162d2788966a1fb6cc3aa2e0c51a93";
		let key_id = "abcdef0123456789";
		let auth_header = format!("HMAC {key_id}:{timestamp}:{hmac}");

		let req = create_test_request(Some(auth_header));

		let result = extract_auth_params(&req);
		assert!(result.is_ok());
		let AuthParams { key_id: kid, timestamp: ts, hmac_hex } = result.unwrap();
		assert_eq!(kid, key_id);
		assert_eq!(ts, timestamp);
		assert_eq!(hmac_hex, hmac);
	}

	#[test]
	fn test_extract_auth_params_missing_header() {
		let req = create_test_request(None);

		let result = extract_auth_params(&req);
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().error_code, AuthError);
	}

	#[test]
	fn test_extract_auth_params_invalid_format() {
		// Missing "HMAC " prefix
		let req = create_test_request(Some("12345:deadbeef".to_string()));

		let result = extract_auth_params(&req);
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().error_code, AuthError);
	}
}
