// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::Arc;

use ldk_node::lightning::routing::gossip::NodeId;
use ldk_server_grpc::api::{GraphGetNodeRequest, GraphGetNodeResponse};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;
use crate::util::proto_adapter::graph_node_to_proto;

pub(crate) async fn handle_graph_get_node_request(
	context: Arc<Context>, request: GraphGetNodeRequest,
) -> Result<GraphGetNodeResponse, LdkServerError> {
	let node_id: NodeId = request.node_id.parse().map_err(|_| {
		LdkServerError::new(
			InvalidRequestError,
			format!("Invalid node_id: {}. Expected a hex-encoded public key.", request.node_id),
		)
	})?;

	let node_info = context.node.network_graph().node(&node_id).ok_or_else(|| {
		LdkServerError::new(
			InvalidRequestError,
			format!("Node with ID {} not found in the network graph.", request.node_id),
		)
	})?;

	let response = GraphGetNodeResponse { node: Some(graph_node_to_proto(node_info)) };
	Ok(response)
}
