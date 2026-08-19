// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::str::FromStr;
use std::sync::Arc;

use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::config::ChannelConfig;
use ldk_node::lightning::ln::msgs::SocketAddress;
use ldk_server_grpc::api::open_channel_request::Amount;
use ldk_server_grpc::api::{OpenChannelRequest, OpenChannelResponse};

use crate::api::error::{LdkServerError, LdkServerErrorCode};
use crate::api::{build_channel_config_from_proto, require_amount};
use crate::service::Context;

pub(crate) async fn handle_open_channel(
	context: Arc<Context>, request: OpenChannelRequest,
) -> Result<OpenChannelResponse, LdkServerError> {
	let node_id = PublicKey::from_str(&request.node_pubkey)
		.map_err(|_| ldk_node::NodeError::InvalidPublicKey)?;
	let address = SocketAddress::from_str(&request.address)
		.map_err(|_| ldk_node::NodeError::InvalidSocketAddress)?;

	let amount = require_amount(request.amount)?;

	let channel_config = request
		.channel_config
		.map(|proto_config| build_channel_config_from_proto(ChannelConfig::default(), proto_config))
		.transpose()?;

	let user_channel_id =
		match (request.announce_channel, request.disable_counterparty_reserve, amount) {
			(true, false, Amount::AllFunds(_)) => context.node.open_announced_channel_with_all(
				node_id,
				address,
				request.push_to_counterparty_msat,
				channel_config,
			)?,
			(true, false, Amount::ChannelAmountSats(amount_sats)) => {
				context.node.open_announced_channel(
					node_id,
					address,
					amount_sats,
					request.push_to_counterparty_msat,
					channel_config,
				)?
			},
			(false, true, Amount::AllFunds(_)) => context.node.open_0reserve_channel_with_all(
				node_id,
				address,
				request.push_to_counterparty_msat,
				channel_config,
			)?,
			(false, true, Amount::ChannelAmountSats(amount_sats)) => {
				context.node.open_0reserve_channel(
					node_id,
					address,
					amount_sats,
					request.push_to_counterparty_msat,
					channel_config,
				)?
			},
			(false, false, Amount::AllFunds(_)) => context.node.open_channel_with_all(
				node_id,
				address,
				request.push_to_counterparty_msat,
				channel_config,
			)?,
			(false, false, Amount::ChannelAmountSats(amount_sats)) => context.node.open_channel(
				node_id,
				address,
				amount_sats,
				request.push_to_counterparty_msat,
				channel_config,
			)?,
			(true, true, _) => {
				return Err(LdkServerError::new(
					LdkServerErrorCode::InvalidRequestError,
					"Cannot set both `announce_channel` and `disable_counterparty_reserve`",
				));
			},
		};

	let response = OpenChannelResponse { user_channel_id: user_channel_id.0.to_string() };
	Ok(response)
}
