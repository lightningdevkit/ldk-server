// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::Arc;

use hex::DisplayHex;
use ldk_node::lightning::blinded_path::message::BlindedMessagePath;
use ldk_node::lightning::util::ser::{Readable, Writeable};
use ldk_server_grpc::api::{
	Bolt12BlindedPathsForAsyncRecipientRequest, Bolt12BlindedPathsForAsyncRecipientResponse,
	Bolt12ReceiveAsyncRequest, Bolt12ReceiveAsyncResponse,
	Bolt12SetPathsToStaticInvoiceServerRequest, Bolt12SetPathsToStaticInvoiceServerResponse,
};

use crate::api::error::LdkServerError;
use crate::service::Context;

pub(crate) async fn handle_bolt12_receive_async_request(
	context: Arc<Context>, _request: Bolt12ReceiveAsyncRequest,
) -> Result<Bolt12ReceiveAsyncResponse, LdkServerError> {
	let offer = context.node.bolt12_payment().receive_async()?;
	let offer_id = offer.id().0.to_lower_hex_string();

	Ok(Bolt12ReceiveAsyncResponse { offer: offer.to_string(), offer_id })
}

pub(crate) async fn handle_bolt12_set_paths_to_static_invoice_server_request(
	context: Arc<Context>, request: Bolt12SetPathsToStaticInvoiceServerRequest,
) -> Result<Bolt12SetPathsToStaticInvoiceServerResponse, LdkServerError> {
	let mut encoded_paths = &request.blinded_paths[..];
	let paths = Vec::<BlindedMessagePath>::read(&mut encoded_paths)
		.map_err(|_| ldk_node::NodeError::InvalidBlindedPaths)?;
	if !encoded_paths.is_empty() {
		return Err(ldk_node::NodeError::InvalidBlindedPaths.into());
	}
	context.node.bolt12_payment().set_paths_to_static_invoice_server(paths)?;

	Ok(Bolt12SetPathsToStaticInvoiceServerResponse {})
}

pub(crate) async fn handle_bolt12_blinded_paths_for_async_recipient_request(
	context: Arc<Context>, request: Bolt12BlindedPathsForAsyncRecipientRequest,
) -> Result<Bolt12BlindedPathsForAsyncRecipientResponse, LdkServerError> {
	let paths = context
		.node
		.bolt12_payment()
		.blinded_paths_for_async_recipient(request.recipient_id.to_vec())?;

	Ok(Bolt12BlindedPathsForAsyncRecipientResponse { blinded_paths: paths.encode().into() })
}
