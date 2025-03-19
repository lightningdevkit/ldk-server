use crate::api::error::LdkServerError;
use crate::service::Context;
use ldk_server_protos::api::{GetMetricsRequest, GetMetricsResponse};

pub(crate) const GET_METRICS: &str = "metrics";

pub(crate) fn handle_metrics_request(
	context: Context, _request: GetMetricsRequest,
) -> Result<GetMetricsResponse, LdkServerError> {
	let metrics = context.prometheus_handle.render();
	Ok(GetMetricsResponse { metrics })
}
