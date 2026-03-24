// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use serde::{Deserialize, Serialize};

/// An event emitted by the LDK Server to notify consumers of payment lifecycle changes.
///
/// Events are published to the configured messaging system (e.g., RabbitMQ) as JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
	PaymentReceived(PaymentReceived),
	PaymentSuccessful(PaymentSuccessful),
	PaymentFailed(PaymentFailed),
	PaymentForwarded(PaymentForwarded),
	PaymentClaimable(PaymentClaimable),
}

/// PaymentReceived indicates a payment has been received.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaymentReceived {
	/// The payment details for the received payment.
	pub payment: super::types::Payment,
}

/// PaymentSuccessful indicates a sent payment was successful.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaymentSuccessful {
	/// The payment details for the successful payment.
	pub payment: super::types::Payment,
}

/// PaymentFailed indicates a sent payment has failed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaymentFailed {
	/// The payment details for the failed payment.
	pub payment: super::types::Payment,
}

/// PaymentClaimable indicates a payment has arrived and is waiting to be manually claimed or failed.
/// This event is only emitted for payments created via `Bolt11ReceiveForHash`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaymentClaimable {
	/// The payment details for the claimable payment.
	pub payment: super::types::Payment,
}

/// PaymentForwarded indicates a payment was forwarded through the node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaymentForwarded {
	/// The forwarded payment details.
	pub forwarded_payment: super::types::ForwardedPayment,
}
