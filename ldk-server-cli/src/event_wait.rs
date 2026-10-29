// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::time::Duration;

use ldk_server_client::client::EventStream;
use ldk_server_client::error::LdkServerError;
use ldk_server_client::error::LdkServerErrorCode::InternalError;
use ldk_server_client::ldk_server_grpc::events::event_envelope::Event;

pub(crate) const EXIT_WAIT_FAILED: i32 = 2;
pub(crate) const EXIT_WAIT_TIMEOUT: i32 = 3;

pub(crate) enum WaitOutcome<T> {
	Matched(T),
	TimedOut,
}

pub(crate) async fn wait_for_event<T>(
	events: &mut EventStream, subject: &str, timeout: Option<Duration>,
	mut matcher: impl FnMut(Event) -> Option<T>,
) -> Result<WaitOutcome<T>, LdkServerError> {
	let deadline = timeout.map(|timeout| tokio::time::Instant::now() + timeout);

	loop {
		let next = events.next_message();
		let message = match deadline {
			Some(deadline) => match tokio::time::timeout_at(deadline, next).await {
				Ok(message) => message,
				Err(_) => return Ok(WaitOutcome::TimedOut),
			},
			None => next.await,
		};

		let envelope = match message {
			Some(Ok(envelope)) => envelope,
			Some(Err(e)) => return Err(e),
			None => {
				return Err(LdkServerError::new(
					InternalError,
					format!("event stream ended before {subject} resolved"),
				))
			},
		};

		if let Some(event) = envelope.event {
			if let Some(matched) = matcher(event) {
				return Ok(WaitOutcome::Matched(matched));
			}
		}
	}
}
