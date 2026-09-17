// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::Arc;

use hex::FromHex;
use ldk_node::lightning::ln::types::ChannelId;
use ldk_server_grpc::api::{GetChannelForwardingStatsRequest, GetChannelForwardingStatsResponse};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;
use crate::util::proto_adapter::channel_forwarding_stats_to_proto;

pub(crate) async fn handle_get_channel_forwarding_stats_request(
	context: Arc<Context>, request: GetChannelForwardingStatsRequest,
) -> Result<GetChannelForwardingStatsResponse, LdkServerError> {
	let id = <[u8; 32]>::from_hex(&request.channel_id).map_err(|_| {
		LdkServerError::new(
			InvalidRequestError,
			"Invalid channel_id, must be a 32-byte hex string.",
		)
	})?;
	let stats = context.node.forwarding_analytics().channel_stats(&ChannelId(id))?;
	Ok(GetChannelForwardingStatsResponse { stats: stats.map(channel_forwarding_stats_to_proto) })
}
