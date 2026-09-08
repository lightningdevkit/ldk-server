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
use ldk_server_grpc::api::{ListPaymentsRequest, ListPaymentsResponse};

use crate::api::error::LdkServerError;
use crate::service::Context;
use crate::util::proto_adapter::payment_to_proto;

pub(crate) async fn handle_list_payments_request(
	context: Arc<Context>, request: ListPaymentsRequest,
) -> Result<ListPaymentsResponse, LdkServerError> {
	let page_token = request.page_token.map(NodePageToken::new);
	let page = context.node.list_payments(page_token)?;

	let response = ListPaymentsResponse {
		payments: page.payments.into_iter().map(payment_to_proto).collect(),
		next_page_token: page.next_page_token.map(|token| token.to_string()),
	};
	Ok(response)
}
