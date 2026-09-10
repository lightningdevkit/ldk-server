// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Server-side macaroon policy, root storage, and RPC authorization.

mod authorization;
mod persistence;
mod policy;
mod store;

use std::io;

pub(crate) use authorization::{method_authorization, MethodAuthorization};
pub(crate) use policy::MacaroonInfo;
#[cfg(test)]
pub(crate) use store::test_util;
pub(crate) use store::MacaroonStore;

use crate::api::error::{LdkServerError, LdkServerErrorCode};

fn invalid_data(message: impl Into<String>) -> io::Error {
	io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn invalid_request(message: impl Into<String>) -> LdkServerError {
	LdkServerError::new(LdkServerErrorCode::InvalidRequestError, message)
}

fn authorization_error(message: impl Into<String>) -> LdkServerError {
	LdkServerError::new(LdkServerErrorCode::AuthorizationError, message)
}

fn store_lock_error() -> LdkServerError {
	internal_error("macaroon store lock is poisoned")
}

fn internal_error(message: impl std::fmt::Display) -> LdkServerError {
	LdkServerError::new(LdkServerErrorCode::InternalServerError, message.to_string())
}

fn auth_error(message: impl Into<String>) -> LdkServerError {
	LdkServerError::new(LdkServerErrorCode::AuthError, message)
}
