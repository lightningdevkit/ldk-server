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

/// Implements the error type ([`LdkServerError`](error::LdkServerError)) returned on interacting with [`LdkServerClient`](client::LdkServerClient).
pub mod error;

/// Request/Response structs required for interacting with the client.
pub use ldk_server_grpc;

#[cfg(feature = "uniffi")]
#[allow(missing_docs)]
pub mod uniffi_types;

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();
