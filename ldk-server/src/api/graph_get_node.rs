// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use ldk_node::lightning::routing::gossip::NodeId;
use ldk_server_json_models::api::{GraphGetNodeRequest, GraphGetNodeResponse};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;
use crate::util::adapter::graph_node_to_model;

pub(crate) fn handle_graph_get_node_request(
	context: Context, request: GraphGetNodeRequest,
) -> Result<GraphGetNodeResponse, LdkServerError> {
	let node_id = NodeId::from_slice(&request.node_id).map_err(|_| {
		LdkServerError::new(
			InvalidRequestError,
			"Invalid node_id: expected a valid 33-byte public key.".to_string(),
		)
	})?;

	let node_info = context.node.network_graph().node(&node_id).ok_or_else(|| {
		LdkServerError::new(InvalidRequestError, "Node not found in the network graph.".to_string())
	})?;

	let response = GraphGetNodeResponse { node: Some(graph_node_to_model(node_info)) };
	Ok(response)
}
