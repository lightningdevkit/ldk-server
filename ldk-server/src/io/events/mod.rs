// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

pub(crate) mod sse;

use ldk_server_json_models::events::Event;

/// Event variant to event name mapping.
pub(crate) fn get_event_name(event: &Event) -> &'static str {
	match event {
		Event::PaymentReceived(_) => "PaymentReceived",
		Event::PaymentSuccessful(_) => "PaymentSuccessful",
		Event::PaymentFailed(_) => "PaymentFailed",
		Event::PaymentForwarded(_) => "PaymentForwarded",
		Event::PaymentClaimable(_) => "PaymentClaimable",
	}
}
