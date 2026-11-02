// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::time::Duration;

use ldk_server_client::client::LdkServerClient;
use ldk_server_client::ldk_server_grpc::api::{
	unified_send_response, GetPaymentDetailsRequest, GetPaymentDetailsResponse, UnifiedSendRequest,
	UnifiedSendResponse,
};
use ldk_server_client::ldk_server_grpc::events::event_envelope::Event;

use crate::event_wait::{wait_for_event, WaitOutcome, EXIT_WAIT_FAILED, EXIT_WAIT_TIMEOUT};
use crate::{handle_error, handle_response_result, print_response, sanitize_for_terminal};

fn lightning_payment_id(response: &UnifiedSendResponse) -> Option<&str> {
	match response.payment_result.as_ref() {
		Some(unified_send_response::PaymentResult::Bolt11PaymentId(id))
		| Some(unified_send_response::PaymentResult::Bolt12PaymentId(id)) => Some(id.as_str()),
		_ => None,
	}
}

fn exit_with_payment(code: i32, message: &str, payment: &GetPaymentDetailsResponse) -> ! {
	eprintln!("{}", sanitize_for_terminal(message.to_string()));
	print_response(payment);
	std::process::exit(code);
}

/// Subscribe before paying, then wait for `PaymentSuccessful` / `PaymentFailed`.
///
/// The event stream is live-only, so the subscription is opened before `unified_send`.
/// A dropped stream is an error; it is not replayed. `timeout` of `None` waits until
/// the payment finishes. On-chain results are printed immediately.
/// Exit codes: 0 succeeded, 2 failed, 3 timed out.
pub(crate) async fn pay_and_wait(
	client: &LdkServerClient, request: UnifiedSendRequest, timeout: Option<Duration>,
) {
	let mut events = client.subscribe_events().await.unwrap_or_else(|e| handle_error(e));

	let response = client.unified_send(request).await.unwrap_or_else(|e| handle_error(e));

	let Some(payment_id) = lightning_payment_id(&response).map(str::to_string) else {
		handle_response_result::<_, UnifiedSendResponse>(Ok(response));
		return;
	};

	match timeout {
		Some(timeout) => eprintln!(
			"Payment initiated: {payment_id} (waiting up to {}s for terminal state)",
			timeout.as_secs()
		),
		None => eprintln!("Payment initiated: {payment_id} (waiting for terminal state)"),
	}

	let outcome = wait_for_event(&mut events, &format!("payment {payment_id}"), timeout, |event| {
		classify(event, &payment_id)
	})
	.await
	.unwrap_or_else(|e| handle_error(e));

	match outcome {
		WaitOutcome::Matched(Ok(payment)) => print_response(&payment),
		WaitOutcome::Matched(Err(payment)) => {
			exit_with_payment(EXIT_WAIT_FAILED, &format!("Payment {payment_id} failed"), &payment);
		},
		WaitOutcome::TimedOut => {
			let secs = timeout.map(|timeout| timeout.as_secs()).unwrap_or(0);
			eprintln!(
				"{}",
				sanitize_for_terminal(format!(
					"Timed out after {secs}s waiting for payment {payment_id}"
				))
			);
			if let Some(payment) = fetch_latest(client, &payment_id).await {
				print_response(&payment);
			}
			std::process::exit(EXIT_WAIT_TIMEOUT);
		},
	}
}

/// `Ok` means the payment succeeded, `Err` that it failed. `None` means the event is for
/// another payment or is not a terminal payment event.
fn classify(
	event: Event, payment_id: &str,
) -> Option<Result<GetPaymentDetailsResponse, GetPaymentDetailsResponse>> {
	match event {
		Event::PaymentSuccessful(event) if event.payment_id == payment_id => {
			Some(Ok(GetPaymentDetailsResponse { payment: event.payment }))
		},
		Event::PaymentFailed(event) if event.payment_id == payment_id => {
			Some(Err(GetPaymentDetailsResponse { payment: event.payment }))
		},
		_ => None,
	}
}

async fn fetch_latest(
	client: &LdkServerClient, payment_id: &str,
) -> Option<GetPaymentDetailsResponse> {
	match client
		.get_payment_details(GetPaymentDetailsRequest { payment_id: payment_id.to_string() })
		.await
	{
		Ok(details) if details.payment.is_some() => Some(details),
		Ok(_) => {
			eprintln!("Payment {payment_id} was not found. It may not be persisted yet.");
			None
		},
		Err(e) => {
			eprintln!("Warning: failed to fetch payment {payment_id}: {}", e.message);
			None
		},
	}
}
