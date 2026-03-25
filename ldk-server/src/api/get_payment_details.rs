// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use ldk_node::lightning::ln::channelmanager::PaymentId;
use ldk_server_json_models::api::{GetPaymentDetailsRequest, GetPaymentDetailsResponse};

use crate::api::error::LdkServerError;
use crate::service::Context;
use crate::util::adapter::payment_to_model;

pub(crate) fn handle_get_payment_details_request(
	context: Context, request: GetPaymentDetailsRequest,
) -> Result<GetPaymentDetailsResponse, LdkServerError> {
	let payment_details = context.node.payment(&PaymentId(request.payment_id));

	let response = GetPaymentDetailsResponse { payment: payment_details.map(payment_to_model) };

	Ok(response)
}
