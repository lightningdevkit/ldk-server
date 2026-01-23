// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use ldk_server_protos::api::{ListPaymentsRequest, ListPaymentsResponse};
use ldk_server_protos::types::{PageToken, Payment};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::InternalServerError;
use crate::io::persist;
use crate::service::Context;
use crate::util::proto_adapter::payment_to_proto;

pub(crate) fn handle_list_payments_request(
	context: Context, request: ListPaymentsRequest,
) -> Result<ListPaymentsResponse, LdkServerError> {
	let page_token = request.page_token.map(|p| (p.token, p.index));

	let list_response =
		persist::list_payments(context.paginated_kv_store, page_token).map_err(|e| {
			LdkServerError::new(InternalServerError, format!("Failed to list payments: {}", e))
		})?;

	let payments: Vec<Payment> = list_response.payments.into_iter().map(payment_to_proto).collect();

	Ok(ListPaymentsResponse {
		payments,
		next_page_token: list_response
			.next_page_token
			.map(|(token, index)| PageToken { token, index }),
	})
}
