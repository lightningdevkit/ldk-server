// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::str::FromStr;

use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::config::ChannelConfig;
use ldk_node::lightning::ln::msgs::SocketAddress;
use ldk_server_protos::api::{OpenChannelRequest, OpenChannelResponse};

use crate::api::build_channel_config_from_proto;
use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;

pub(crate) fn handle_open_channel(
	context: Context, request: OpenChannelRequest,
) -> Result<OpenChannelResponse, LdkServerError> {
	let node_id = PublicKey::from_str(&request.node_pubkey)
		.map_err(|_| ldk_node::NodeError::InvalidPublicKey)?;
	let address = match request.address {
		Some(address) => {
			SocketAddress::from_str(&address).map_err(|_| ldk_node::NodeError::InvalidSocketAddress)?
		},
		None => context
			.node
			.list_peers()
			.into_iter()
			.find(|peer| peer.node_id == node_id)
			.map(|peer| peer.address)
			.ok_or_else(|| {
				LdkServerError::new(
					InvalidRequestError,
					"Address is required unless the peer is currently connected. Provide an address or connect-peer first.".to_string(),
				)
			})?,
	};

	let channel_config = request
		.channel_config
		.map(|proto_config| build_channel_config_from_proto(ChannelConfig::default(), proto_config))
		.transpose()?;

	let user_channel_id = if request.announce_channel {
		context.node.open_announced_channel(
			node_id,
			address,
			request.channel_amount_sats,
			request.push_to_counterparty_msat,
			channel_config,
		)?
	} else {
		context.node.open_channel(
			node_id,
			address,
			request.channel_amount_sats,
			request.push_to_counterparty_msat,
			channel_config,
		)?
	};

	let response = OpenChannelResponse { user_channel_id: user_channel_id.0.to_string() };
	Ok(response)
}
