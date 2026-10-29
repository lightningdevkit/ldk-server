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
use ldk_server_client::ldk_server_grpc::api::{ListChannelsRequest, OpenChannelRequest};
use ldk_server_client::ldk_server_grpc::events::event_envelope::Event;
use ldk_server_client::ldk_server_grpc::events::{ChannelState, ChannelStateChanged};
use ldk_server_client::ldk_server_grpc::types::Channel;
use serde_json::json;

use crate::event_wait::{wait_for_event, WaitOutcome, EXIT_WAIT_FAILED, EXIT_WAIT_TIMEOUT};
use crate::{handle_error, print_response, sanitize_for_terminal};

/// Subscribe before opening, then wait until the funding transaction is broadcast or the open
/// fails.
///
/// The event stream is live-only, so the subscription is opened before `open_channel`. A dropped
/// stream is an error; it is not replayed. `timeout` of `None` waits until the open resolves.
/// This does not wait for the funding transaction to confirm.
/// Exit codes: 0 pending, 1 request or event stream error, 2 failed, 3 timed out.
pub(crate) async fn open_channel_and_wait(
	client: &LdkServerClient, request: OpenChannelRequest, timeout: Option<Duration>,
) {
	let mut events = client.subscribe_events().await.unwrap_or_else(|e| handle_error(e));

	let response = client.open_channel(request).await.unwrap_or_else(|e| handle_error(e));
	let user_channel_id = response.user_channel_id;

	let waiting_for = match timeout {
		Some(timeout) => format!("waiting up to {}s for the counterparty", timeout.as_secs()),
		None => "waiting for the counterparty".to_string(),
	};
	eprintln!(
		"{}",
		sanitize_for_terminal(format!("Channel open initiated: {user_channel_id} ({waiting_for})"))
	);

	let outcome =
		wait_for_event(&mut events, &format!("channel open {user_channel_id}"), timeout, |event| {
			classify(event, &user_channel_id)
		})
		.await
		.unwrap_or_else(|e| handle_error(e));

	match outcome {
		WaitOutcome::Matched(Outcome::Pending(event)) => print_response(&json!({
			"user_channel_id": event.user_channel_id,
			"channel_id": event.channel_id,
			"funding_txo": event.funding_txo,
		})),
		WaitOutcome::Matched(Outcome::Failed(event)) => {
			let reason = event
				.reason
				.map(|reason| reason.message)
				.filter(|message| !message.is_empty())
				.unwrap_or_else(|| "no reason given".to_string());
			eprintln!(
				"{}",
				sanitize_for_terminal(format!("Channel open {user_channel_id} failed: {reason}"))
			);
			std::process::exit(EXIT_WAIT_FAILED);
		},
		WaitOutcome::TimedOut => {
			let secs = timeout.unwrap_or_default().as_secs();
			eprintln!(
				"{}",
				sanitize_for_terminal(format!(
					"Timed out after {secs}s waiting for channel open {user_channel_id}"
				))
			);
			match fetch_channel(client, &user_channel_id).await {
				Some(channel) => print_response(&channel),
				None => eprintln!(
					"{}",
					sanitize_for_terminal(format!(
						"Channel {user_channel_id} is not listed. The open may have failed; check the server log."
					))
				),
			}
			std::process::exit(EXIT_WAIT_TIMEOUT);
		},
	}
}

enum Outcome {
	/// The funding transaction was broadcast.
	Pending(ChannelStateChanged),
	Failed(ChannelStateChanged),
}

/// `None` means the event is for another channel or is not a final state for the open.
fn classify(event: Event, user_channel_id: &str) -> Option<Outcome> {
	let Event::ChannelStateChanged(event) = event else {
		return None;
	};

	if event.user_channel_id != user_channel_id {
		return None;
	}
	match event.state() {
		// `Ready` only arrives first if the `Pending` event was missed.
		ChannelState::Pending | ChannelState::Ready => Some(Outcome::Pending(event)),
		ChannelState::OpenFailed | ChannelState::Closed => Some(Outcome::Failed(event)),
		ChannelState::Unspecified => None,
	}
}

async fn fetch_channel(client: &LdkServerClient, user_channel_id: &str) -> Option<Channel> {
	match client.list_channels(ListChannelsRequest {}).await {
		Ok(response) => {
			response.channels.into_iter().find(|channel| channel.user_channel_id == user_channel_id)
		},
		Err(e) => {
			eprintln!(
				"{}",
				sanitize_for_terminal(format!("Warning: failed to list channels: {}", e.message))
			);
			None
		},
	}
}
