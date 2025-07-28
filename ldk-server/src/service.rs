use ldk_node::Node;

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::service::Service;
use hyper::{Request, Response, StatusCode};

use prost::Message;

use crate::api::bolt11_receive::{BOLT11_RECEIVE_PATH, handle_bolt11_receive_request};
use crate::api::bolt11_send::{BOLT11_SEND_PATH, handle_bolt11_send_request};
use crate::api::bolt12_receive::{BOLT12_RECEIVE_PATH, handle_bolt12_receive_request};
use crate::api::bolt12_send::{BOLT12_SEND_PATH, handle_bolt12_send_request};
use crate::api::close_channel::{
	CLOSE_CHANNEL_PATH, FORCE_CLOSE_CHANNEL_PATH, handle_close_channel_request,
	handle_force_close_channel_request,
};
use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::api::get_balances::{GET_BALANCES, handle_get_balances_request};
use crate::api::get_node_info::{GET_NODE_INFO, handle_get_node_info_request};
use crate::api::get_payment_details::{
	GET_PAYMENT_DETAILS_PATH, handle_get_payment_details_request,
};
use crate::api::list_channels::{LIST_CHANNELS_PATH, handle_list_channels_request};
use crate::api::list_forwarded_payments::{
	LIST_FORWARDED_PAYMENTS_PATH, handle_list_forwarded_payments_request,
};
use crate::api::list_payments::{LIST_PAYMENTS_PATH, handle_list_payments_request};
use crate::api::onchain_receive::{ONCHAIN_RECEIVE_PATH, handle_onchain_receive_request};
use crate::api::onchain_send::{ONCHAIN_SEND_PATH, handle_onchain_send_request};
use crate::api::open_channel::{OPEN_CHANNEL_PATH, handle_open_channel};
use crate::api::update_channel_config::{
	UPDATE_CHANNEL_CONFIG_PATH, handle_update_channel_config_request,
};
use crate::io::persist::paginated_kv_store::PaginatedKVStore;
use crate::util::proto_adapter::to_error_response;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Clone)]
pub struct NodeService {
	node: Arc<Node>,
	paginated_kv_store: Arc<dyn PaginatedKVStore>,
}

impl NodeService {
	pub(crate) fn new(node: Arc<Node>, paginated_kv_store: Arc<dyn PaginatedKVStore>) -> Self {
		Self { node, paginated_kv_store }
	}
}

pub(crate) struct Context {
	pub(crate) node: Arc<Node>,
	pub(crate) paginated_kv_store: Arc<dyn PaginatedKVStore>,
}

impl Service<Request<Incoming>> for NodeService {
	type Response = Response<Full<Bytes>>;
	type Error = hyper::Error;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

	fn call(&self, req: Request<Incoming>) -> Self::Future {
		let context = Context {
			node: Arc::clone(&self.node),
			paginated_kv_store: Arc::clone(&self.paginated_kv_store),
		};
		// Exclude '/' from path pattern matching.
		match &req.uri().path()[1..] {
			GET_NODE_INFO => Box::pin(handle_request(context, req, handle_get_node_info_request)),
			GET_BALANCES => Box::pin(handle_request(context, req, handle_get_balances_request)),
			ONCHAIN_RECEIVE_PATH => {
				Box::pin(handle_request(context, req, handle_onchain_receive_request))
			},
			ONCHAIN_SEND_PATH => {
				Box::pin(handle_request(context, req, handle_onchain_send_request))
			},
			BOLT11_RECEIVE_PATH => {
				Box::pin(handle_request(context, req, handle_bolt11_receive_request))
			},
			BOLT11_SEND_PATH => Box::pin(handle_request(context, req, handle_bolt11_send_request)),
			BOLT12_RECEIVE_PATH => {
				Box::pin(handle_request(context, req, handle_bolt12_receive_request))
			},
			BOLT12_SEND_PATH => Box::pin(handle_request(context, req, handle_bolt12_send_request)),
			OPEN_CHANNEL_PATH => Box::pin(handle_request(context, req, handle_open_channel)),
			CLOSE_CHANNEL_PATH => {
				Box::pin(handle_request(context, req, handle_close_channel_request))
			},
			FORCE_CLOSE_CHANNEL_PATH => {
				Box::pin(handle_request(context, req, handle_force_close_channel_request))
			},
			LIST_CHANNELS_PATH => {
				Box::pin(handle_request(context, req, handle_list_channels_request))
			},
			UPDATE_CHANNEL_CONFIG_PATH => {
				Box::pin(handle_request(context, req, handle_update_channel_config_request))
			},
			GET_PAYMENT_DETAILS_PATH => {
				Box::pin(handle_request(context, req, handle_get_payment_details_request))
			},
			LIST_PAYMENTS_PATH => {
				Box::pin(handle_request(context, req, handle_list_payments_request))
			},
			LIST_FORWARDED_PAYMENTS_PATH => {
				Box::pin(handle_request(context, req, handle_list_forwarded_payments_request))
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
	context: Context, request: Request<Incoming>, handler: F,
) -> Result<<NodeService as Service<Request<Incoming>>>::Response, hyper::Error> {
	// TODO: we should bound the amount of data we read to avoid allocating too much memory.
	let bytes = request.into_body().collect().await?.to_bytes();
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
