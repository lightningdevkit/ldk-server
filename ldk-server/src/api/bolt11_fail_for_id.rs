// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::Arc;

use ldk_server_grpc::api::{Bolt11FailForIdRequest, Bolt11FailForIdResponse};

use crate::api::error::LdkServerError;
use crate::service::Context;

pub(crate) async fn handle_bolt11_fail_for_id_request(
	context: Arc<Context>, request: Bolt11FailForIdRequest,
) -> Result<Bolt11FailForIdResponse, LdkServerError> {
	let payment_id = crate::api::parse_payment_id(&request.payment_id)?;

	context.node.bolt11_payment().fail_for_id(payment_id)?;

	Ok(Bolt11FailForIdResponse {})
}
