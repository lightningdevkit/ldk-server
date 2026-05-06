// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

/// EventEnvelope wraps different event types in a single message to be used by EventPublisher.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EventEnvelope {
	#[prost(oneof = "event_envelope::Event", tags = "2, 3, 4, 6, 7, 8")]
	pub event: ::core::option::Option<event_envelope::Event>,
}
/// Nested message and enum types in `EventEnvelope`.
pub mod event_envelope {
	#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
	}
}
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CounterpartyForceClosedDetails {
	#[prost(string, tag = "1")]
	pub peer_msg: ::prost::alloc::string::String,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HolderForceClosedDetails {
	#[prost(bool, optional, tag = "1")]
	pub broadcasted_latest_txn: ::core::option::Option<bool>,
	#[prost(string, tag = "2")]
	pub message: ::prost::alloc::string::String,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProcessingErrorDetails {
	#[prost(string, tag = "1")]
	pub err: ::prost::alloc::string::String,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HtlcsTimedOutDetails {
	#[prost(string, optional, tag = "1")]
	pub payment_hash: ::core::option::Option<::prost::alloc::string::String>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PeerFeerateTooLowDetails {
	#[prost(uint32, tag = "1")]
	pub peer_feerate_sat_per_kw: u32,
	#[prost(uint32, tag = "2")]
	pub required_feerate_sat_per_kw: u32,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
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
	#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
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
}
/// PaymentReceived indicates a payment has been received.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PaymentReceived {
	/// The payment details for the payment in event.
	#[prost(message, optional, tag = "1")]
	pub payment: ::core::option::Option<super::types::Payment>,
}
/// PaymentSuccessful indicates a sent payment was successful.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PaymentSuccessful {
	/// The payment details for the payment in event.
	#[prost(message, optional, tag = "1")]
	pub payment: ::core::option::Option<super::types::Payment>,
}
/// PaymentFailed indicates a sent payment has failed.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PaymentFailed {
	/// The payment details for the payment in event.
	#[prost(message, optional, tag = "1")]
	pub payment: ::core::option::Option<super::types::Payment>,
}
/// PaymentClaimable indicates a payment has arrived and is waiting to be manually claimed or failed.
/// This event is only emitted for payments created via `Bolt11ReceiveForHash`.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PaymentClaimable {
	/// The payment details for the claimable payment.
	#[prost(message, optional, tag = "1")]
	pub payment: ::core::option::Option<super::types::Payment>,
}
/// PaymentForwarded indicates a payment was forwarded through the node.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PaymentForwarded {
	#[prost(message, optional, tag = "1")]
	pub forwarded_payment: ::core::option::Option<super::types::ForwardedPayment>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
