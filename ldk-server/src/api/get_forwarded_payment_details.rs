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

use ldk_node::payment::ForwardedPaymentId;
use ldk_server_grpc::api::{GetForwardedPaymentDetailsRequest, GetForwardedPaymentDetailsResponse};

use crate::api::error::LdkServerError;
use crate::service::Context;
use crate::util::proto_adapter::forwarded_payment_to_proto;

pub(crate) async fn handle_get_forwarded_payment_details_request(
	context: Arc<Context>, request: GetForwardedPaymentDetailsRequest,
) -> Result<GetForwardedPaymentDetailsResponse, LdkServerError> {
	let id = ForwardedPaymentId::from_str(&request.forwarded_payment_id)?;
	let payment = context.node.forwarding_analytics().payment(&id)?;
	Ok(GetForwardedPaymentDetailsResponse { payment: payment.map(forwarded_payment_to_proto) })
}
