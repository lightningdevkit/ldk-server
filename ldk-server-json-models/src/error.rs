// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// When HttpStatusCode is not ok (200), the response `content` contains a serialized `ErrorResponse`
/// with the relevant ErrorCode and `message`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
	/// The error message containing a generic description of the error condition in English.
	/// It is intended for a human audience only and should not be parsed to extract any information
	/// programmatically. Client-side code may use it for logging only.
	pub message: String,
	/// The error code uniquely identifying an error condition.
	/// It is meant to be read and understood programmatically by code that detects/handles errors by
	/// type.
	pub error_code: ErrorCode,
}

#[derive(
	Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema,
)]
pub enum ErrorCode {
	/// Will never be used as `error_code` by server.
	UnknownError,
	/// Used in the following cases:
	///    - The request was missing a required argument.
	///    - The specified argument was invalid, incomplete or in the wrong format.
	///    - The request body of api cannot be deserialized.
	///    - The request does not follow api contract.
	InvalidRequestError,
	/// Used when authentication fails or in case of an unauthorized request.
	AuthError,
	/// Used to represent an error while doing a Lightning operation.
	LightningError,
	/// Used when an internal server error occurred. The client is probably at no fault.
	InternalServerError,
}
