// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::body::{Bytes, Frame};
use ldk_server_json_models::events::Event;
use log::warn;
use tokio::sync::{broadcast, mpsc};

use super::get_event_name;

/// An HTTP body that streams Server-Sent Events from a broadcast channel.
///
/// Uses an internal mpsc channel bridged from the broadcast receiver via a
/// spawned task, so that `poll_frame` can poll a `Receiver` directly.
pub(crate) struct SseBody {
	receiver: mpsc::Receiver<Event>,
}

impl SseBody {
	pub(crate) fn new(broadcast_rx: broadcast::Receiver<Event>) -> Self {
		let (tx, rx) = mpsc::channel(64);
		tokio::spawn(bridge_broadcast_to_mpsc(broadcast_rx, tx));
		Self { receiver: rx }
	}
}

async fn bridge_broadcast_to_mpsc(
	mut broadcast_rx: broadcast::Receiver<Event>, tx: mpsc::Sender<Event>,
) {
	loop {
		match broadcast_rx.recv().await {
			Ok(event) => {
				if tx.send(event).await.is_err() {
					break;
				}
			},
			Err(broadcast::error::RecvError::Lagged(n)) => {
				warn!("SSE subscriber lagged, skipped {n} events");
			},
			Err(broadcast::error::RecvError::Closed) => break,
		}
	}
}

impl hyper::body::Body for SseBody {
	type Data = Bytes;
	type Error = std::convert::Infallible;

	fn poll_frame(
		mut self: Pin<&mut Self>, cx: &mut Context<'_>,
	) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
		match self.receiver.poll_recv(cx) {
			Poll::Ready(Some(event)) => {
				let encoded = format_sse_event(&event);
				Poll::Ready(Some(Ok(Frame::data(Bytes::from(encoded)))))
			},
			Poll::Ready(None) => Poll::Ready(None),
			Poll::Pending => Poll::Pending,
		}
	}
}

fn format_sse_event(event: &Event) -> String {
	let event_name = get_event_name(event);
	let data = serde_json::to_string(event).unwrap();
	format!("event: {event_name}\ndata: {data}\n\n")
}
