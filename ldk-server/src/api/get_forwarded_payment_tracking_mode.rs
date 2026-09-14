// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::Arc;

use ldk_node::config::ForwardedPaymentTrackingMode as NodeMode;
use ldk_server_grpc::api::{
	GetForwardedPaymentTrackingModeRequest, GetForwardedPaymentTrackingModeResponse,
};
use ldk_server_grpc::types::ForwardedPaymentTrackingMode;

use crate::api::error::LdkServerError;
use crate::service::Context;

pub(crate) async fn handle_get_forwarded_payment_tracking_mode_request(
	context: Arc<Context>, _request: GetForwardedPaymentTrackingModeRequest,
) -> Result<GetForwardedPaymentTrackingModeResponse, LdkServerError> {
	let mode = match context.node.forwarding_analytics().tracking_mode() {
		NodeMode::Stats => ForwardedPaymentTrackingMode::Stats,
		NodeMode::Detailed => ForwardedPaymentTrackingMode::Detailed,
	};
	Ok(GetForwardedPaymentTrackingModeResponse { mode: mode as i32 })
}
