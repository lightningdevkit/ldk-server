// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::time::Duration;

use ldk_server_client::client::{EventStream, LdkServerClient};
use ldk_server_client::error::LdkServerError;
use ldk_server_client::error::LdkServerErrorCode::InternalError;
use ldk_server_client::ldk_server_grpc::api::unified_send_response;
use ldk_server_client::ldk_server_grpc::api::{
	GetPaymentDetailsRequest, GetPaymentDetailsResponse, UnifiedSendRequest, UnifiedSendResponse,
};
use ldk_server_client::ldk_server_grpc::events::event_envelope::Event;

use crate::{handle_error, handle_response_result, print_response, sanitize_for_terminal};

const EXIT_PAYMENT_FAILED: i32 = 2;
const EXIT_PAYMENT_TIMEOUT: i32 = 3;

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
	let mut events = client.subscribe_events().await.map_err(handle_error).unwrap();

	let response = client.unified_send(request).await.map_err(handle_error).unwrap();

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

	match wait_for_terminal_event(client, &mut events, &payment_id, timeout).await {
		Ok(payment) => print_response(&payment),
		Err(WaitError::Failed(payment)) => {
			exit_with_payment(
				EXIT_PAYMENT_FAILED,
				&format!("Payment {payment_id} failed"),
				&payment,
			);
		},
		Err(WaitError::TimedOut(payment)) => {
			let secs = timeout.map(|timeout| timeout.as_secs()).unwrap_or(0);
			eprintln!(
				"{}",
				sanitize_for_terminal(format!(
					"Timed out after {secs}s waiting for payment {payment_id}"
				))
			);
			if let Some(payment) = payment {
				print_response(&payment);
			}
			std::process::exit(EXIT_PAYMENT_TIMEOUT);
		},
		Err(WaitError::Transport(e)) => handle_error(e),
	}
}

enum WaitError {
	Failed(GetPaymentDetailsResponse),
	TimedOut(Option<GetPaymentDetailsResponse>),
	Transport(LdkServerError),
}

async fn wait_for_terminal_event(
	client: &LdkServerClient, events: &mut EventStream, payment_id: &str, timeout: Option<Duration>,
) -> Result<GetPaymentDetailsResponse, WaitError> {
	let deadline = timeout.map(|timeout| tokio::time::Instant::now() + timeout);

	match next_matching_event(events, payment_id, deadline).await? {
		Some(Ok(payment)) => Ok(payment),
		Some(Err(WaitError::TimedOut(_))) => {
			Err(WaitError::TimedOut(fetch_latest(client, payment_id).await))
		},
		Some(Err(error)) => Err(error),
		None => Err(WaitError::Transport(LdkServerError::new(
			InternalError,
			format!("event stream ended before payment {payment_id} reached a terminal state"),
		))),
	}
}

/// `Ok(None)` means the stream ended without a matching terminal event.
async fn next_matching_event(
	events: &mut EventStream, payment_id: &str, deadline: Option<tokio::time::Instant>,
) -> Result<Option<Result<GetPaymentDetailsResponse, WaitError>>, WaitError> {
	loop {
		let next = events.next_message();
		let message = match deadline {
			Some(deadline) => {
				let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
				if remaining.is_zero() {
					return Ok(Some(Err(WaitError::TimedOut(None))));
				}
				match tokio::time::timeout(remaining, next).await {
					Ok(message) => message,
					Err(_) => return Ok(Some(Err(WaitError::TimedOut(None)))),
				}
			},
			None => next.await,
		};

		match message {
			Some(Ok(envelope)) => match envelope.event {
				Some(Event::PaymentSuccessful(event)) if event.payment_id == payment_id => {
					return Ok(Some(Ok(GetPaymentDetailsResponse { payment: event.payment })));
				},
				Some(Event::PaymentFailed(event)) if event.payment_id == payment_id => {
					return Ok(Some(Err(WaitError::Failed(GetPaymentDetailsResponse {
						payment: event.payment,
					}))));
				},
				_ => {},
			},
			Some(Err(e)) => return Err(WaitError::Transport(e)),
			None => return Ok(None),
		}
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
