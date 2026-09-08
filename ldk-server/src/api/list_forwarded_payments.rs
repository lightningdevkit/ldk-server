// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::Arc;

use bytes::Bytes;
use ldk_server_grpc::api::{ListForwardedPaymentsRequest, ListForwardedPaymentsResponse};
use ldk_server_grpc::types::ForwardedPayment;
use prost::Message;

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::{InternalServerError, InvalidRequestError};
use crate::io::persist::{
	FORWARDED_PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
	FORWARDED_PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
};
use crate::service::Context;

pub(crate) async fn handle_list_forwarded_payments_request(
	context: Arc<Context>, request: ListForwardedPaymentsRequest,
) -> Result<ListForwardedPaymentsResponse, LdkServerError> {
	let page_token = request.page_token.map(parse_page_token).transpose()?;
	let list_response = context
		.paginated_kv_store
		.list(
			FORWARDED_PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
			FORWARDED_PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
			page_token,
		)
		.map_err(|e| {
			LdkServerError::new(
				InternalServerError,
				format!("Failed to list forwarded payments: {}", e),
			)
		})?;

	let mut forwarded_payments: Vec<ForwardedPayment> =
		Vec::with_capacity(list_response.keys.len());
	for key in list_response.keys {
		let forwarded_payment_bytes = context
			.paginated_kv_store
			.read(
				FORWARDED_PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
				FORWARDED_PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
				&key,
			)
			.map_err(|e| {
				LdkServerError::new(
					InternalServerError,
					format!("Failed to read forwarded payment data: {}", e),
				)
			})?;
		let forwarded_payment = ForwardedPayment::decode(Bytes::from(forwarded_payment_bytes))
			.map_err(|e| {
				LdkServerError::new(
					InternalServerError,
					format!("Failed to decode forwarded payment: {}", e),
				)
			})?;
		forwarded_payments.push(forwarded_payment);
	}
	let response = ListForwardedPaymentsResponse {
		forwarded_payments,
		next_page_token: list_response.next_page_token.map(format_page_token),
	};
	Ok(response)
}

fn parse_page_token(page_token: String) -> Result<(String, i64), LdkServerError> {
	let (token, index) = page_token.rsplit_once(':').ok_or_else(invalid_page_token)?;
	let index = index.parse::<i64>().map_err(|_| invalid_page_token())?;
	Ok((token.to_string(), index))
}

fn format_page_token((token, index): (String, i64)) -> String {
	format!("{token}:{index}")
}

fn invalid_page_token() -> LdkServerError {
	LdkServerError::new(InvalidRequestError, "Invalid page token".to_string())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn page_token_round_trip_preserves_colons() {
		let token = ("store:v2:cursor".to_string(), 7);
		assert_eq!(parse_page_token(format_page_token(token.clone())).unwrap(), token);
	}

	#[test]
	fn parse_page_token_rejects_invalid_index() {
		let error = parse_page_token("cursor:not-an-index".to_string()).unwrap_err();
		assert_eq!(error.error_code, InvalidRequestError);
	}
}
