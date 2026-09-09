// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

#![doc = include_str!("../README.md")]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(missing_docs)]

mod credential;
mod macaroon;

pub use credential::{
	bind_macaroon_to_request, bind_macaroon_to_request_at, derive_macaroon, parse_reusable_macaroon,
};
pub use macaroon::{
	Macaroon, RequestBinding, MAX_CAVEATS, MAX_MACAROON_BYTES, REQUEST_CAVEAT_PREFIX,
	REQUEST_TIMESTAMP_TOLERANCE_SECS,
};
