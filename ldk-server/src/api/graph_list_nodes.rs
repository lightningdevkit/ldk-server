// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use ldk_server_protos::api::{GraphListNodesRequest, GraphListNodesResponse};

use crate::api::error::LdkServerError;
use crate::service::Context;

pub(crate) fn handle_graph_list_nodes_request(
	context: Context, _request: GraphListNodesRequest,
) -> Result<GraphListNodesResponse, LdkServerError> {
	let node_ids =
		context.node.network_graph().list_nodes().into_iter().map(|n| n.to_string()).collect();

	let response = GraphListNodesResponse { node_ids };
	Ok(response)
}
