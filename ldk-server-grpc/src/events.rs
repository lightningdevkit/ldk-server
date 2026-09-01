// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

/// EventEnvelope wraps different event types in a single message to be used by EventPublisher.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EventEnvelope {
	#[prost(oneof = "event_envelope::Event", tags = "2, 3, 4, 6, 7, 8, 9, 10")]
	pub event: ::core::option::Option<event_envelope::Event>,
}
/// Nested message and enum types in `EventEnvelope`.
pub mod event_envelope {
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
	#[allow(clippy::derive_partial_eq_without_eq)]
	#[derive(Clone, PartialEq, ::prost::Oneof)]
	pub enum Event {
		#[prost(message, tag = "2")]
		PaymentReceived(super::PaymentReceived),
		#[prost(message, tag = "3")]
		PaymentSuccessful(super::PaymentSuccessful),
		#[prost(message, tag = "4")]
		PaymentFailed(super::PaymentFailed),
		#[prost(message, tag = "6")]
		PaymentForwarded(super::PaymentForwarded),
		#[prost(message, tag = "7")]
		PaymentClaimable(super::PaymentClaimable),
		#[prost(message, tag = "8")]
		ChannelStateChanged(super::ChannelStateChanged),
		#[prost(message, tag = "9")]
		SpliceNegotiated(super::SpliceNegotiated),
		#[prost(message, tag = "10")]
		SpliceNegotiationFailed(super::SpliceNegotiationFailed),
	}
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CounterpartyForceClosedDetails {
	#[prost(string, tag = "1")]
	pub peer_msg: ::prost::alloc::string::String,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HolderForceClosedDetails {
	#[prost(bool, optional, tag = "1")]
	pub broadcasted_latest_txn: ::core::option::Option<bool>,
	#[prost(string, tag = "2")]
	pub message: ::prost::alloc::string::String,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProcessingErrorDetails {
	#[prost(string, tag = "1")]
	pub err: ::prost::alloc::string::String,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HtlcsTimedOutDetails {
	#[prost(string, optional, tag = "1")]
	pub payment_hash: ::core::option::Option<::prost::alloc::string::String>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PeerFeerateTooLowDetails {
	#[prost(uint32, tag = "1")]
	pub peer_feerate_sat_per_kw: u32,
	#[prost(uint32, tag = "2")]
	pub required_feerate_sat_per_kw: u32,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChannelStateChangeReason {
	#[prost(enumeration = "ChannelStateChangeReasonKind", tag = "1")]
	pub kind: i32,
	#[prost(string, tag = "2")]
	pub message: ::prost::alloc::string::String,
	#[prost(oneof = "channel_state_change_reason::Details", tags = "3, 4, 5, 6, 7")]
	pub details: ::core::option::Option<channel_state_change_reason::Details>,
}
/// Nested message and enum types in `ChannelStateChangeReason`.
pub mod channel_state_change_reason {
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
	#[allow(clippy::derive_partial_eq_without_eq)]
	#[derive(Clone, PartialEq, ::prost::Oneof)]
	pub enum Details {
		#[prost(message, tag = "3")]
		CounterpartyForceClosed(super::CounterpartyForceClosedDetails),
		#[prost(message, tag = "4")]
		HolderForceClosed(super::HolderForceClosedDetails),
		#[prost(message, tag = "5")]
		ProcessingError(super::ProcessingErrorDetails),
		#[prost(message, tag = "6")]
		HtlcsTimedOut(super::HtlcsTimedOutDetails),
		#[prost(message, tag = "7")]
		PeerFeerateTooLow(super::PeerFeerateTooLowDetails),
	}
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChannelStateChanged {
	#[prost(string, tag = "1")]
	pub channel_id: ::prost::alloc::string::String,
	#[prost(string, tag = "2")]
	pub user_channel_id: ::prost::alloc::string::String,
	#[prost(string, optional, tag = "3")]
	pub counterparty_node_id: ::core::option::Option<::prost::alloc::string::String>,
	#[prost(enumeration = "ChannelState", tag = "4")]
	pub state: i32,
	#[prost(string, optional, tag = "5")]
	pub funding_txo: ::core::option::Option<::prost::alloc::string::String>,
	#[prost(message, optional, tag = "6")]
	pub reason: ::core::option::Option<ChannelStateChangeReason>,
	#[prost(enumeration = "ChannelClosureInitiator", tag = "7")]
	pub closure_initiator: i32,
	/// The `temporary_channel_id` this channel used to be known by during channel establishment.
	///
	/// Only set when `state` is `CHANNEL_STATE_PENDING`.
	#[prost(string, optional, tag = "8")]
	pub former_temporary_channel_id: ::core::option::Option<::prost::alloc::string::String>,
}
/// SpliceNegotiated indicates a channel splice has been negotiated and the funding
/// transaction is pending confirmation on-chain.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpliceNegotiated {
	#[prost(string, tag = "1")]
	pub channel_id: ::prost::alloc::string::String,
	#[prost(string, tag = "2")]
	pub user_channel_id: ::prost::alloc::string::String,
	#[prost(string, tag = "3")]
	pub counterparty_node_id: ::prost::alloc::string::String,
	/// The outpoint of the channel's splice funding transaction.
	#[prost(string, tag = "4")]
	pub new_funding_txo: ::prost::alloc::string::String,
}
/// SpliceNegotiationFailed indicates a channel splice negotiation round has failed.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpliceNegotiationFailed {
	#[prost(string, tag = "1")]
	pub channel_id: ::prost::alloc::string::String,
	#[prost(string, tag = "2")]
	pub user_channel_id: ::prost::alloc::string::String,
	#[prost(string, tag = "3")]
	pub counterparty_node_id: ::prost::alloc::string::String,
}
/// PaymentReceived indicates a payment has been received.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PaymentReceived {
	/// The payment details for the payment in event.
	#[prost(message, optional, tag = "1")]
	pub payment: ::core::option::Option<super::types::Payment>,
	/// Custom TLV records attached to the incoming payment, if any.
	#[prost(message, repeated, tag = "2")]
	pub custom_records: ::prost::alloc::vec::Vec<super::types::CustomTlvRecord>,
}
/// PaymentSuccessful indicates a sent payment was successful.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PaymentSuccessful {
	/// The payment details for the payment in event.
	#[prost(message, optional, tag = "1")]
	pub payment: ::core::option::Option<super::types::Payment>,
}
/// PaymentFailed indicates a sent payment has failed.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PaymentFailed {
	/// The payment details for the payment in event.
	#[prost(message, optional, tag = "1")]
	pub payment: ::core::option::Option<super::types::Payment>,
	/// The reason the payment failed, if known.
	///
	/// This is only available on the emitted event; `GetPaymentDetails` cannot
	/// recover it as LDK Node does not currently persist the failure reason in
	/// `PaymentDetails`.
	#[prost(enumeration = "PaymentFailureReason", optional, tag = "2")]
	pub reason: ::core::option::Option<i32>,
}
/// PaymentClaimable indicates a payment has arrived and is waiting to be manually claimed or failed.
/// This event is only emitted for payments created via `Bolt11ReceiveForHash`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PaymentClaimable {
	/// The payment details for the claimable payment.
	#[prost(message, optional, tag = "1")]
	pub payment: ::core::option::Option<super::types::Payment>,
	/// Custom TLV records attached to the claimable payment, if any.
	#[prost(message, repeated, tag = "2")]
	pub custom_records: ::prost::alloc::vec::Vec<super::types::CustomTlvRecord>,
	/// The block height by which this payment must be claimed before it is failed back.
	#[prost(uint32, optional, tag = "3")]
	pub claim_deadline: ::core::option::Option<u32>,
}
/// PaymentForwarded indicates a payment was forwarded through the node.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "serde", serde(default))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PaymentForwarded {
	#[prost(message, optional, tag = "1")]
	pub forwarded_payment: ::core::option::Option<super::types::ForwardedPayment>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ChannelState {
	Unspecified = 0,
	Pending = 1,
	Ready = 2,
	OpenFailed = 3,
	Closed = 4,
}
impl ChannelState {
	/// String value of the enum field names used in the ProtoBuf definition.
	///
	/// The values are not transformed in any way and thus are considered stable
	/// (if the ProtoBuf definition does not change) and safe for programmatic use.
	pub fn as_str_name(&self) -> &'static str {
		match self {
			ChannelState::Unspecified => "CHANNEL_STATE_UNSPECIFIED",
			ChannelState::Pending => "CHANNEL_STATE_PENDING",
			ChannelState::Ready => "CHANNEL_STATE_READY",
			ChannelState::OpenFailed => "CHANNEL_STATE_OPEN_FAILED",
			ChannelState::Closed => "CHANNEL_STATE_CLOSED",
		}
	}
	/// Creates an enum from field names used in the ProtoBuf definition.
	pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
		match value {
			"CHANNEL_STATE_UNSPECIFIED" => Some(Self::Unspecified),
			"CHANNEL_STATE_PENDING" => Some(Self::Pending),
			"CHANNEL_STATE_READY" => Some(Self::Ready),
			"CHANNEL_STATE_OPEN_FAILED" => Some(Self::OpenFailed),
			"CHANNEL_STATE_CLOSED" => Some(Self::Closed),
			_ => None,
		}
	}
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ChannelClosureInitiator {
	Unspecified = 0,
	Local = 1,
	Remote = 2,
	Unknown = 3,
}
impl ChannelClosureInitiator {
	/// String value of the enum field names used in the ProtoBuf definition.
	///
	/// The values are not transformed in any way and thus are considered stable
	/// (if the ProtoBuf definition does not change) and safe for programmatic use.
	pub fn as_str_name(&self) -> &'static str {
		match self {
			ChannelClosureInitiator::Unspecified => "CHANNEL_CLOSURE_INITIATOR_UNSPECIFIED",
			ChannelClosureInitiator::Local => "CHANNEL_CLOSURE_INITIATOR_LOCAL",
			ChannelClosureInitiator::Remote => "CHANNEL_CLOSURE_INITIATOR_REMOTE",
			ChannelClosureInitiator::Unknown => "CHANNEL_CLOSURE_INITIATOR_UNKNOWN",
		}
	}
	/// Creates an enum from field names used in the ProtoBuf definition.
	pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
		match value {
			"CHANNEL_CLOSURE_INITIATOR_UNSPECIFIED" => Some(Self::Unspecified),
			"CHANNEL_CLOSURE_INITIATOR_LOCAL" => Some(Self::Local),
			"CHANNEL_CLOSURE_INITIATOR_REMOTE" => Some(Self::Remote),
			"CHANNEL_CLOSURE_INITIATOR_UNKNOWN" => Some(Self::Unknown),
			_ => None,
		}
	}
}
/// PaymentFailureReason mirrors LDK's `lightning::events::PaymentFailureReason`,
/// indicating why a sent payment failed.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum PaymentFailureReason {
	Unspecified = 0,
	/// The intended recipient rejected our payment.
	RecipientRejected = 1,
	/// The user chose to abandon this payment by calling `abandon_payment`.
	UserAbandoned = 2,
	/// We exhausted all of our retry attempts while trying to send the payment,
	/// or we exhausted the configured retry timeout.
	RetriesExhausted = 3,
	/// Either the BOLT12 invoice was expired by the time we received it or the
	/// payment expired while retrying.
	PaymentExpired = 4,
	/// We failed to find a route while sending or retrying the payment.
	RouteNotFound = 5,
	/// An unexpected error occurred, generally indicating a problem with the router.
	UnexpectedError = 6,
	/// An invoice was received that required unknown features.
	UnknownRequiredFeatures = 7,
	/// A BOLT12 invoice was not received in a reasonable amount of time.
	InvoiceRequestExpired = 8,
	/// An invoice request for the payment was rejected by the recipient.
	InvoiceRequestRejected = 9,
	/// Failed to create a blinded path back to ourselves.
	BlindedPathCreationFailed = 10,
}
impl PaymentFailureReason {
	/// String value of the enum field names used in the ProtoBuf definition.
	///
	/// The values are not transformed in any way and thus are considered stable
	/// (if the ProtoBuf definition does not change) and safe for programmatic use.
	pub fn as_str_name(&self) -> &'static str {
		match self {
			PaymentFailureReason::Unspecified => "PAYMENT_FAILURE_REASON_UNSPECIFIED",
			PaymentFailureReason::RecipientRejected => "PAYMENT_FAILURE_REASON_RECIPIENT_REJECTED",
			PaymentFailureReason::UserAbandoned => "PAYMENT_FAILURE_REASON_USER_ABANDONED",
			PaymentFailureReason::RetriesExhausted => "PAYMENT_FAILURE_REASON_RETRIES_EXHAUSTED",
			PaymentFailureReason::PaymentExpired => "PAYMENT_FAILURE_REASON_PAYMENT_EXPIRED",
			PaymentFailureReason::RouteNotFound => "PAYMENT_FAILURE_REASON_ROUTE_NOT_FOUND",
			PaymentFailureReason::UnexpectedError => "PAYMENT_FAILURE_REASON_UNEXPECTED_ERROR",
			PaymentFailureReason::UnknownRequiredFeatures => {
				"PAYMENT_FAILURE_REASON_UNKNOWN_REQUIRED_FEATURES"
			},
			PaymentFailureReason::InvoiceRequestExpired => {
				"PAYMENT_FAILURE_REASON_INVOICE_REQUEST_EXPIRED"
			},
			PaymentFailureReason::InvoiceRequestRejected => {
				"PAYMENT_FAILURE_REASON_INVOICE_REQUEST_REJECTED"
			},
			PaymentFailureReason::BlindedPathCreationFailed => {
				"PAYMENT_FAILURE_REASON_BLINDED_PATH_CREATION_FAILED"
			},
		}
	}
	/// Creates an enum from field names used in the ProtoBuf definition.
	pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
		match value {
			"PAYMENT_FAILURE_REASON_UNSPECIFIED" => Some(Self::Unspecified),
			"PAYMENT_FAILURE_REASON_RECIPIENT_REJECTED" => Some(Self::RecipientRejected),
			"PAYMENT_FAILURE_REASON_USER_ABANDONED" => Some(Self::UserAbandoned),
			"PAYMENT_FAILURE_REASON_RETRIES_EXHAUSTED" => Some(Self::RetriesExhausted),
			"PAYMENT_FAILURE_REASON_PAYMENT_EXPIRED" => Some(Self::PaymentExpired),
			"PAYMENT_FAILURE_REASON_ROUTE_NOT_FOUND" => Some(Self::RouteNotFound),
			"PAYMENT_FAILURE_REASON_UNEXPECTED_ERROR" => Some(Self::UnexpectedError),
			"PAYMENT_FAILURE_REASON_UNKNOWN_REQUIRED_FEATURES" => {
				Some(Self::UnknownRequiredFeatures)
			},
			"PAYMENT_FAILURE_REASON_INVOICE_REQUEST_EXPIRED" => Some(Self::InvoiceRequestExpired),
			"PAYMENT_FAILURE_REASON_INVOICE_REQUEST_REJECTED" => Some(Self::InvoiceRequestRejected),
			"PAYMENT_FAILURE_REASON_BLINDED_PATH_CREATION_FAILED" => {
				Some(Self::BlindedPathCreationFailed)
			},
			_ => None,
		}
	}
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ChannelStateChangeReasonKind {
	Unspecified = 0,
	CounterpartyForceClosed = 1,
	HolderForceClosed = 2,
	LegacyCooperativeClosure = 3,
	CounterpartyInitiatedCooperativeClosure = 4,
	LocallyInitiatedCooperativeClosure = 5,
	CommitmentTxConfirmed = 6,
	FundingTimedOut = 7,
	ProcessingError = 8,
	DisconnectedPeer = 9,
	OutdatedChannelManager = 10,
	CounterpartyCoopClosedUnfundedChannel = 11,
	LocallyCoopClosedUnfundedChannel = 12,
	FundingBatchClosure = 13,
	HtlcsTimedOut = 14,
	PeerFeerateTooLow = 15,
}
impl ChannelStateChangeReasonKind {
	/// String value of the enum field names used in the ProtoBuf definition.
	///
	/// The values are not transformed in any way and thus are considered stable
	/// (if the ProtoBuf definition does not change) and safe for programmatic use.
	pub fn as_str_name(&self) -> &'static str {
		match self {
			ChannelStateChangeReasonKind::Unspecified => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_UNSPECIFIED"
			},
			ChannelStateChangeReasonKind::CounterpartyForceClosed => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_COUNTERPARTY_FORCE_CLOSED"
			},
			ChannelStateChangeReasonKind::HolderForceClosed => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_HOLDER_FORCE_CLOSED"
			},
			ChannelStateChangeReasonKind::LegacyCooperativeClosure => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_LEGACY_COOPERATIVE_CLOSURE"
			},
			ChannelStateChangeReasonKind::CounterpartyInitiatedCooperativeClosure => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_COUNTERPARTY_INITIATED_COOPERATIVE_CLOSURE"
			},
			ChannelStateChangeReasonKind::LocallyInitiatedCooperativeClosure => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_LOCALLY_INITIATED_COOPERATIVE_CLOSURE"
			},
			ChannelStateChangeReasonKind::CommitmentTxConfirmed => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_COMMITMENT_TX_CONFIRMED"
			},
			ChannelStateChangeReasonKind::FundingTimedOut => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_FUNDING_TIMED_OUT"
			},
			ChannelStateChangeReasonKind::ProcessingError => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_PROCESSING_ERROR"
			},
			ChannelStateChangeReasonKind::DisconnectedPeer => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_DISCONNECTED_PEER"
			},
			ChannelStateChangeReasonKind::OutdatedChannelManager => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_OUTDATED_CHANNEL_MANAGER"
			},
			ChannelStateChangeReasonKind::CounterpartyCoopClosedUnfundedChannel => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_COUNTERPARTY_COOP_CLOSED_UNFUNDED_CHANNEL"
			},
			ChannelStateChangeReasonKind::LocallyCoopClosedUnfundedChannel => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_LOCALLY_COOP_CLOSED_UNFUNDED_CHANNEL"
			},
			ChannelStateChangeReasonKind::FundingBatchClosure => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_FUNDING_BATCH_CLOSURE"
			},
			ChannelStateChangeReasonKind::HtlcsTimedOut => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_HTLCS_TIMED_OUT"
			},
			ChannelStateChangeReasonKind::PeerFeerateTooLow => {
				"CHANNEL_STATE_CHANGE_REASON_KIND_PEER_FEERATE_TOO_LOW"
			},
		}
	}
	/// Creates an enum from field names used in the ProtoBuf definition.
	pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
		match value {
			"CHANNEL_STATE_CHANGE_REASON_KIND_UNSPECIFIED" => Some(Self::Unspecified),
			"CHANNEL_STATE_CHANGE_REASON_KIND_COUNTERPARTY_FORCE_CLOSED" => {
				Some(Self::CounterpartyForceClosed)
			},
			"CHANNEL_STATE_CHANGE_REASON_KIND_HOLDER_FORCE_CLOSED" => Some(Self::HolderForceClosed),
			"CHANNEL_STATE_CHANGE_REASON_KIND_LEGACY_COOPERATIVE_CLOSURE" => {
				Some(Self::LegacyCooperativeClosure)
			},
			"CHANNEL_STATE_CHANGE_REASON_KIND_COUNTERPARTY_INITIATED_COOPERATIVE_CLOSURE" => {
				Some(Self::CounterpartyInitiatedCooperativeClosure)
			},
			"CHANNEL_STATE_CHANGE_REASON_KIND_LOCALLY_INITIATED_COOPERATIVE_CLOSURE" => {
				Some(Self::LocallyInitiatedCooperativeClosure)
			},
			"CHANNEL_STATE_CHANGE_REASON_KIND_COMMITMENT_TX_CONFIRMED" => {
				Some(Self::CommitmentTxConfirmed)
			},
			"CHANNEL_STATE_CHANGE_REASON_KIND_FUNDING_TIMED_OUT" => Some(Self::FundingTimedOut),
			"CHANNEL_STATE_CHANGE_REASON_KIND_PROCESSING_ERROR" => Some(Self::ProcessingError),
			"CHANNEL_STATE_CHANGE_REASON_KIND_DISCONNECTED_PEER" => Some(Self::DisconnectedPeer),
			"CHANNEL_STATE_CHANGE_REASON_KIND_OUTDATED_CHANNEL_MANAGER" => {
				Some(Self::OutdatedChannelManager)
			},
			"CHANNEL_STATE_CHANGE_REASON_KIND_COUNTERPARTY_COOP_CLOSED_UNFUNDED_CHANNEL" => {
				Some(Self::CounterpartyCoopClosedUnfundedChannel)
			},
			"CHANNEL_STATE_CHANGE_REASON_KIND_LOCALLY_COOP_CLOSED_UNFUNDED_CHANNEL" => {
				Some(Self::LocallyCoopClosedUnfundedChannel)
			},
			"CHANNEL_STATE_CHANGE_REASON_KIND_FUNDING_BATCH_CLOSURE" => {
				Some(Self::FundingBatchClosure)
			},
			"CHANNEL_STATE_CHANGE_REASON_KIND_HTLCS_TIMED_OUT" => Some(Self::HtlcsTimedOut),
			"CHANNEL_STATE_CHANGE_REASON_KIND_PEER_FEERATE_TOO_LOW" => {
				Some(Self::PeerFeerateTooLow)
			},
			_ => None,
		}
	}
}
