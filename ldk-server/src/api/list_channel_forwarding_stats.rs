// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::Arc;

use ldk_node::payment::PageToken;
use ldk_server_grpc::api::{ListChannelForwardingStatsRequest, ListChannelForwardingStatsResponse};

use crate::api::error::LdkServerError;
use crate::service::Context;
use crate::util::proto_adapter::channel_forwarding_stats_to_proto;

pub(crate) async fn handle_list_channel_forwarding_stats_request(
	context: Arc<Context>, request: ListChannelForwardingStatsRequest,
) -> Result<ListChannelForwardingStatsResponse, LdkServerError> {
	let page = context
		.node
		.forwarding_analytics()
		.list_channel_stats(request.page_token.map(PageToken::new))?;
	Ok(ListChannelForwardingStatsResponse {
		stats: page.stats.into_iter().map(channel_forwarding_stats_to_proto).collect(),
		next_page_token: page.next_page_token.map(|token| token.to_string()),
	})
}
