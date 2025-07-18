use hyper::body::Incoming;
use hyper::Request;

use crate::api::error::LdkServerError;
use crate::service::Context;

pub(crate) const GET_METRICS: &str = "metrics";

pub(crate) fn handle_metrics_request(
	context: Context, _request: Request<Incoming>,
) -> Result<String, LdkServerError> {
	let metrics = context.prometheus_handle.render();
	Ok(metrics)
}
