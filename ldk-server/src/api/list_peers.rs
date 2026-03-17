// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use ldk_server_protos::api::{ListPeersRequest, ListPeersResponse};

use crate::api::error::LdkServerError;
use crate::service::Context;
use crate::util::proto_adapter::peer_to_proto;

pub(crate) fn handle_list_peers_request(
	context: Context, _request: ListPeersRequest,
) -> Result<ListPeersResponse, LdkServerError> {
	let peers = context.node.list_peers().into_iter().map(peer_to_proto).collect();

	let response = ListPeersResponse { peers };
	Ok(response)
}
