// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

pub(crate) mod paginated_kv_store;
pub(crate) mod sqlite_store;
pub(crate) mod types;

use std::io;
use std::sync::Arc;

use ldk_node::lightning::util::ser::Readable;
use ldk_node::payment::PaymentDetails;

use paginated_kv_store::PaginatedKVStore;
use types::StoredForwardedPayment;

/// The forwarded payments will be persisted under this prefix.
pub(crate) const FORWARDED_PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE: &str = "forwarded_payments";
pub(crate) const FORWARDED_PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE: &str = "";

/// The payments will be persisted under this prefix.
pub(crate) const PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE: &str = "payments";
pub(crate) const PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE: &str = "";

/// Response from listing payments.
pub(crate) struct ListPaymentsResponse {
	pub payments: Vec<PaymentDetails>,
	pub next_page_token: Option<(String, i64)>,
}

/// Response from listing forwarded payments.
pub(crate) struct ListForwardedPaymentsResponse {
	pub forwarded_payments: Vec<StoredForwardedPayment>,
	pub next_page_token: Option<(String, i64)>,
}

/// List and deserialize payments from the store.
pub(crate) fn list_payments(
	store: Arc<dyn PaginatedKVStore>, page_token: Option<(String, i64)>,
) -> Result<ListPaymentsResponse, io::Error> {
	let list_response = store.list(
		PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
		PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
		page_token,
	)?;

	let mut payments = Vec::with_capacity(list_response.keys.len());
	for key in list_response.keys {
		let payment_bytes = store.read(
			PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
			PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
			&key,
		)?;

		let mut cursor = io::Cursor::new(&payment_bytes);
		let payment = PaymentDetails::read(&mut cursor).map_err(|e| {
			io::Error::new(io::ErrorKind::InvalidData, format!("Failed to decode payment: {}", e))
		})?;
		payments.push(payment);
	}

	Ok(ListPaymentsResponse { payments, next_page_token: list_response.next_page_token })
}

/// List and deserialize forwarded payments from the store.
pub(crate) fn list_forwarded_payments(
	store: Arc<dyn PaginatedKVStore>, page_token: Option<(String, i64)>,
) -> Result<ListForwardedPaymentsResponse, io::Error> {
	let list_response = store.list(
		FORWARDED_PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
		FORWARDED_PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
		page_token,
	)?;

	let mut forwarded_payments = Vec::with_capacity(list_response.keys.len());
	for key in list_response.keys {
		let payment_bytes = store.read(
			FORWARDED_PAYMENTS_PERSISTENCE_PRIMARY_NAMESPACE,
			FORWARDED_PAYMENTS_PERSISTENCE_SECONDARY_NAMESPACE,
			&key,
		)?;

		let mut cursor = io::Cursor::new(&payment_bytes);
		let payment = StoredForwardedPayment::read(&mut cursor).map_err(|e| {
			io::Error::new(
				io::ErrorKind::InvalidData,
				format!("Failed to decode forwarded payment: {}", e),
			)
		})?;
		forwarded_payments.push(payment);
	}

	Ok(ListForwardedPaymentsResponse {
		forwarded_payments,
		next_page_token: list_response.next_page_token,
	})
}
