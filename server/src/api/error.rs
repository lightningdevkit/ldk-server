
pub(crate) struct LdkServerError {
	// The error message containing a generic description of the error condition in English.
	// It is intended for a human audience only and should not be parsed to extract any information
	// programmatically. Client-side code may use it for logging only.
	pub(crate) message: String,

	// The error code uniquely identifying an error condition.
	// It is meant to be read and understood programmatically by code that detects/handles errors by
	// type.
	pub(crate) error_code: LdkServerErrorCode,


	// The `sub_error_code` used to represent further details of `Error` while doing Lightning operation.
	// It is only set when `error_code` is set to `LightningError`.
	pub(crate) sub_error_code: Option<LightningErrorCode>
}

pub(crate) enum LdkServerErrorCode {
	/// Please refer to [`protos::error::InvalidRequestError`].
	InvalidRequestError,

	/// Please refer to [`protos::error::AuthError`].
	AuthError,

	/// Please refer to [`protos::error::LightningError`].
	LightningError,

	/// Please refer to [`protos::error::InternalServerError`].
	InternalServerError,

	/// There is an unknown error, it could be a client-side bug, unrecognized error-code, network error
	/// or something else.
	InternalError,
}

// TODO: Add docs.
pub(crate) enum LightningErrorCode {
	UnknownLightningError,
	OperationFailed,
	OperationTimedOut,
	PaymentSendingFailed,
	InsufficientFunds,
	DuplicatePayment,
	LiquidityRequestFailed,
	LiquiditySourceUnavailable,
	LiquidityFeeHigh,
}
