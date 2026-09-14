// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::Arc;

use ldk_node::payment::PageToken as NodePageToken;
use ldk_server_grpc::api::{ListForwardedPaymentsRequest, ListForwardedPaymentsResponse};

use crate::api::error::LdkServerError;
use crate::service::Context;
use crate::util::proto_adapter::forwarded_payment_to_proto;

pub(crate) async fn handle_list_forwarded_payments_request(
	context: Arc<Context>, request: ListForwardedPaymentsRequest,
) -> Result<ListForwardedPaymentsResponse, LdkServerError> {
	let page_token = request.page_token.map(NodePageToken::new);
	let page = context.node.forwarding_analytics().list_payments(page_token)?;

	Ok(ListForwardedPaymentsResponse {
		forwarded_payments: page.payments.into_iter().map(forwarded_payment_to_proto).collect(),
		next_page_token: page.next_page_token.map(|token| token.to_string()),
	})
}
