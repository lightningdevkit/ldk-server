/// When HttpStatusCode is not ok (200), the response `content` contains a serialized `ErrorResponse`
/// with the relevant ErrorCode and `message`
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ErrorResponse {
    /// The error message containing a generic description of the error condition in English.
    /// It is intended for a human audience only and should not be parsed to extract any information
    /// programmatically. Client-side code may use it for logging only.
    #[prost(string, tag="1")]
    pub message: ::prost::alloc::string::String,
    /// The error code uniquely identifying an error condition.
    /// It is meant to be read and understood programmatically by code that detects/handles errors by
    /// type.
    ///
    /// **Caution**: If a new type of `error_code` is introduced in oneof, `error_code` field will be unset.
    /// If unset, it should be treated as `UnknownError`, it will not be set as `UnknownError`.
    #[prost(oneof="error_response::ErrorCode", tags="2, 3, 4, 5, 6")]
    pub error_code: ::core::option::Option<error_response::ErrorCode>,
}
/// Nested message and enum types in `ErrorResponse`.
pub mod error_response {
    /// The error code uniquely identifying an error condition.
    /// It is meant to be read and understood programmatically by code that detects/handles errors by
    /// type.
    ///
    /// **Caution**: If a new type of `error_code` is introduced in oneof, `error_code` field will be unset.
    /// If unset, it should be treated as `UnknownError`, it will not be set as `UnknownError`.
    #[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum ErrorCode {
        /// Will neve be used as `error_code` by server.
        #[prost(message, tag="2")]
        UnknownError(super::UnknownError),
        /// Used in the following cases:
        ///    - The request was missing a required argument.
        ///    - The specified argument was invalid, incomplete or in the wrong format.
        ///    - The request body of api cannot be deserialized into corresponding protobuf object.
        ///    - The request does not follow api contract.
        #[prost(message, tag="3")]
        InvalidRequestError(super::InvalidRequestError),
        /// Used when authentication fails or in case of an unauthorized request.
        #[prost(message, tag="4")]
        AuthError(super::AuthError),
        /// Used to represent an Error while doing Lightning operation. Contains `LightningErrorCode` for further details.
        #[prost(message, tag="5")]
        LightningError(super::LightningError),
        /// Used when an internal server error occurred, client is probably at no fault and can safely retry
        /// this error with exponential backoff.
        #[prost(message, tag="6")]
        InternalServerError(super::InternalServerError),
    }
}
/// Will neve be used as `error_code` by server.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UnknownError {
}
/// Used in the following cases:
///    - The request was missing a required argument.
///    - The specified argument was invalid, incomplete or in the wrong format.
///    - The request body of api cannot be deserialized into corresponding protobuf object.
///    - The request does not follow api contract.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InvalidRequestError {
}
/// Used when authentication fails or in case of an unauthorized request.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AuthError {
}
/// Used to represent an Error while doing Lightning operation. Contains `LightningErrorCode` for further details.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LightningError {
    #[prost(enumeration="LightningErrorCode", tag="1")]
    pub lightning_error_code: i32,
}
/// Used when an internal server error occurred, client is probably at no fault and can safely retry
/// this error with exponential backoff.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InternalServerError {
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum LightningErrorCode {
    /// Default protobuf Enum value. Will not be used as `LightningErrorCode` by server.
    /// **Caution**: If a new Enum value is introduced, it will be seen as `UNKNOWN_LIGHTNING_ERROR` by code using earlier
    /// versions of protobuf definition for deserialization.
    UnknownLightningError = 0,
    /// The requested operation failed, such as invoice creation failed, refund creation failed etc.
    OperationFailed = 1,
    /// There was a timeout during the requested operation.
    OperationTimedOut = 2,
    /// Sending a payment has failed.
    PaymentSendingFailed = 3,
    /// The available funds are insufficient to complete the given operation.
    InsufficientFunds = 4,
    /// A payment failed since it has already been initiated.
    DuplicatePayment = 5,
    /// A liquidity request operation failed.
    LiquidityRequestFailed = 6,
    /// The given operation failed due to the required liquidity source being unavailable.
    LiquiditySourceUnavailable = 7,
    /// The given operation failed due to the LSP's required opening fee being too high.
    LiquidityFeeHigh = 8,
}
impl LightningErrorCode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            LightningErrorCode::UnknownLightningError => "UNKNOWN_LIGHTNING_ERROR",
            LightningErrorCode::OperationFailed => "OPERATION_FAILED",
            LightningErrorCode::OperationTimedOut => "OPERATION_TIMED_OUT",
            LightningErrorCode::PaymentSendingFailed => "PAYMENT_SENDING_FAILED",
            LightningErrorCode::InsufficientFunds => "INSUFFICIENT_FUNDS",
            LightningErrorCode::DuplicatePayment => "DUPLICATE_PAYMENT",
            LightningErrorCode::LiquidityRequestFailed => "LIQUIDITY_REQUEST_FAILED",
            LightningErrorCode::LiquiditySourceUnavailable => "LIQUIDITY_SOURCE_UNAVAILABLE",
            LightningErrorCode::LiquidityFeeHigh => "LIQUIDITY_FEE_HIGH",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UNKNOWN_LIGHTNING_ERROR" => Some(Self::UnknownLightningError),
            "OPERATION_FAILED" => Some(Self::OperationFailed),
            "OPERATION_TIMED_OUT" => Some(Self::OperationTimedOut),
            "PAYMENT_SENDING_FAILED" => Some(Self::PaymentSendingFailed),
            "INSUFFICIENT_FUNDS" => Some(Self::InsufficientFunds),
            "DUPLICATE_PAYMENT" => Some(Self::DuplicatePayment),
            "LIQUIDITY_REQUEST_FAILED" => Some(Self::LiquidityRequestFailed),
            "LIQUIDITY_SOURCE_UNAVAILABLE" => Some(Self::LiquiditySourceUnavailable),
            "LIQUIDITY_FEE_HIGH" => Some(Self::LiquidityFeeHigh),
            _ => None,
        }
    }
}
