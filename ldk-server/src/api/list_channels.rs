use crate::api::error::LdkServerError;
use crate::service::Context;
use crate::util::proto_adapter::channel_to_proto;
use ldk_server_protos::api::{ListChannelsRequest, ListChannelsResponse};

pub(crate) fn handle_list_channels_request(
	context: Context, _request: ListChannelsRequest,
) -> Result<ListChannelsResponse, LdkServerError> {
	let channels = context.node.list_channels().into_iter().map(channel_to_proto).collect();

	let response = ListChannelsResponse { channels };
	Ok(response)
}
