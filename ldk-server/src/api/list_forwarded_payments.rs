use crate::io::{
	FORWARDED_PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
	FORWARDED_PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
};
use crate::service::Context;
use bytes::Bytes;
use ldk_server_protos::api::{ListForwardedPaymentsRequest, ListForwardedPaymentsResponse};
use ldk_server_protos::types::{ForwardedPayment, PageToken};
use prost::Message;

pub(crate) const LIST_FORWARDED_PAYMENTS_PATH: &str = "ListForwardedPayments";

pub(crate) fn handle_list_forwarded_payments_request(
	context: Context, request: ListForwardedPaymentsRequest,
) -> Result<ListForwardedPaymentsResponse, ldk_node::NodeError> {
	let page_token = request.page_token.map(|p| (p.token, p.index));
	let list_response = context
		.paginated_kv_store
		.list(
			FORWARDED_PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
			FORWARDED_PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
			page_token,
		)
		.map_err(|_| ldk_node::NodeError::ConnectionFailed)?;

	let mut forwarded_payments: Vec<ForwardedPayment> = vec![];
	for key in list_response.keys {
		let forwarded_payment_bytes = context
			.paginated_kv_store
			.read(
				FORWARDED_PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
				FORWARDED_PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
				&key,
			)
			.map_err(|_| ldk_node::NodeError::ConnectionFailed)?;
		let forwarded_payment = ForwardedPayment::decode(Bytes::from(forwarded_payment_bytes))
			.map_err(|_| ldk_node::NodeError::ConnectionFailed)?;
		forwarded_payments.push(forwarded_payment);
	}
	let response = ListForwardedPaymentsResponse {
		forwarded_payments,
		next_page_token: list_response
			.next_page_token
			.map(|(token, index)| PageToken { token, index }),
	};
	Ok(response)
}
