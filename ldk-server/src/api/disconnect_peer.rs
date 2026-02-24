// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT
// or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::str::FromStr;

use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_server_protos::api::{DisconnectPeerRequest, DisconnectPeerResponse};

use crate::api::error::LdkServerError;
use crate::service::Context;

pub(crate) fn handle_disconnect_peer(
	context: Context, request: DisconnectPeerRequest,
) -> Result<DisconnectPeerResponse, LdkServerError> {
	let node_id = PublicKey::from_str(&request.node_pubkey)
		.map_err(|_| ldk_node::NodeError::InvalidPublicKey)?;

	context.node.disconnect(node_id)?;

	Ok(DisconnectPeerResponse {})
}
