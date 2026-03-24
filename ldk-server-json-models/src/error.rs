// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use serde::{Deserialize, Serialize};

/// When HttpStatusCode is not ok (200), the response `content` contains a serialized `ErrorResponse`
/// with the relevant ErrorCode and `message`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

#[cfg(test)]
mod tests {
	use super::{ErrorCode, ErrorResponse};

	#[test]
	fn error_response_serializes_error_code_in_snake_case() {
		let response = ErrorResponse {
			message: "bad request".to_string(),
			error_code: ErrorCode::InvalidRequestError,
		};

		let value = serde_json::to_value(response).unwrap();

		assert_eq!(value["error_code"], "invalid_request_error");
	}

	#[test]
	fn error_response_roundtrip() {
		let err = ErrorResponse {
			message: "something went wrong".into(),
			error_code: ErrorCode::InternalServerError,
		};
		let json = serde_json::to_value(&err).unwrap();
		assert_eq!(json["error_code"], "internal_server_error");
		let back: ErrorResponse = serde_json::from_value(json).unwrap();
		assert_eq!(back, err);
	}

	#[test]
	fn error_code_all_variants() {
		for (variant, expected) in [
			(ErrorCode::UnknownError, "unknown_error"),
			(ErrorCode::InvalidRequestError, "invalid_request_error"),
			(ErrorCode::AuthError, "auth_error"),
			(ErrorCode::LightningError, "lightning_error"),
			(ErrorCode::InternalServerError, "internal_server_error"),
		] {
			let json = serde_json::to_value(&variant).unwrap();
			assert_eq!(json, expected);
			let back: ErrorCode = serde_json::from_value(json).unwrap();
			assert_eq!(back, variant);
		}
	}
}
