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
#![deny(rustdoc::private_intra_doc_links)]
#![deny(missing_docs)]

/// Implements a [`LdkServerClient`](client::LdkServerClient) to access a hosted instance of LDK Server.
pub mod client;

/// Shared configuration loading and credential resolution logic reused by `ldk-server` clients.
#[cfg(feature = "serde")]
pub mod config;

/// Implements the error type ([`LdkServerError`](error::LdkServerError)) returned on interacting with [`LdkServerClient`](client::LdkServerClient).
pub mod error;

/// Request/Response structs required for interacting with the client.
pub use ldk_server_grpc;

/// Default maximum total CLTV expiry delta for payment routing.
pub const DEFAULT_MAX_TOTAL_CLTV_EXPIRY_DELTA: u32 = 1008;
/// Default maximum number of payment paths.
pub const DEFAULT_MAX_PATH_COUNT: u32 = 10;
/// Default maximum channel saturation power of half.
pub const DEFAULT_MAX_CHANNEL_SATURATION_POWER_OF_HALF: u32 = 2;
/// Default BOLT11 invoice expiry in seconds (24 hours).
pub const DEFAULT_EXPIRY_SECS: u32 = 86_400;
