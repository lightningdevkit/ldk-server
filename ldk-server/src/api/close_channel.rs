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
use ldk_node::UserChannelId;
use ldk_server_protos::api::{
	CloseChannelRequest, CloseChannelResponse, ForceCloseChannelRequest, ForceCloseChannelResponse,
};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InvalidRequestError;
use crate::service::Context;

pub(crate) fn handle_close_channel_request(
	context: Context, request: CloseChannelRequest,
) -> Result<CloseChannelResponse, LdkServerError> {
	let user_channel_id = parse_user_channel_id(&request.user_channel_id)?;
	let counterparty_node_id =
		resolve_counterparty_node_id(&context, &user_channel_id, request.counterparty_node_id)?;

	context.node.close_channel(&user_channel_id, counterparty_node_id)?;

	Ok(CloseChannelResponse {})
}

pub(crate) fn handle_force_close_channel_request(
	context: Context, request: ForceCloseChannelRequest,
) -> Result<ForceCloseChannelResponse, LdkServerError> {
	let user_channel_id = parse_user_channel_id(&request.user_channel_id)?;
	let counterparty_node_id =
		resolve_counterparty_node_id(&context, &user_channel_id, request.counterparty_node_id)?;

	context.node.force_close_channel(
		&user_channel_id,
		counterparty_node_id,
		request.force_close_reason,
	)?;

	Ok(ForceCloseChannelResponse {})
}

fn parse_user_channel_id(id: &str) -> Result<UserChannelId, LdkServerError> {
	let parsed = id.parse::<u128>().map_err(|_| {
		LdkServerError::new(InvalidRequestError, "Invalid UserChannelId.".to_string())
	})?;
	Ok(UserChannelId(parsed))
}

fn parse_counterparty_node_id(id: &str) -> Result<PublicKey, LdkServerError> {
	PublicKey::from_str(id).map_err(|e| {
		LdkServerError::new(
			InvalidRequestError,
			format!("Invalid counterparty node ID, error: {}", e),
		)
	})
}

fn resolve_counterparty_node_id(
	context: &Context, user_channel_id: &UserChannelId, counterparty_node_id: Option<String>,
) -> Result<PublicKey, LdkServerError> {
	match counterparty_node_id {
		Some(id) => parse_counterparty_node_id(&id),
		None => context
			.node
			.list_channels()
			.into_iter()
			.find(|c| c.user_channel_id == *user_channel_id)
			.map(|c| c.counterparty_node_id)
			.ok_or_else(|| {
				LdkServerError::new(
					InvalidRequestError,
					"Channel not found for given user_channel_id.".to_string(),
				)
			}),
	}
}
