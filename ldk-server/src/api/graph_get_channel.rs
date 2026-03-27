// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use ldk_server_grpc::api::{GraphGetChannelRequest, GraphGetChannelResponse};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;
use crate::util::proto_adapter::graph_channel_to_proto;
use std::sync::Arc;

pub(crate) async fn handle_graph_get_channel_request(
	context: Arc<Context>, request: GraphGetChannelRequest,
) -> Result<GraphGetChannelResponse, LdkServerError> {
	let channel_info =
		context.node.network_graph().channel(request.short_channel_id).ok_or_else(|| {
			LdkServerError::new(
				InvalidRequestError,
				format!(
					"Channel with short_channel_id {} not found in the network graph.",
					request.short_channel_id
				),
			)
		})?;

	let response = GraphGetChannelResponse { channel: Some(graph_channel_to_proto(channel_info)) };
	Ok(response)
}
