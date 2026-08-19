// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! CLI-specific type wrappers for API responses.
//!
//! This file contains wrapper types that customize the serialization format
//! of API responses for CLI output. These wrappers ensure that the CLI's output
//! format matches what users expect and what the CLI can parse back as input.

use std::fmt;
use std::str::FromStr;

use hex_conservative::{DisplayHex, FromHex};
use ldk_server_client::ldk_server_grpc::types::{ForwardedPayment, PageToken, Payment};
use serde::Serialize;

/// CLI-specific wrapper for paginated responses that formats the page token
/// as "token:idx" instead of a JSON object.
#[derive(Debug, Clone, Serialize)]
pub struct CliPaginatedResponse<T> {
	/// List of items.
	pub list: Vec<T>,
	/// Next page token formatted as "token:idx", or None if no more pages.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub next_page_token: Option<String>,
}

impl<T> CliPaginatedResponse<T> {
	pub fn new(list: Vec<T>, next_page_token: Option<PageToken>) -> Self {
		Self { list, next_page_token: next_page_token.map(format_page_token) }
	}
}

pub type CliListPaymentsResponse = CliPaginatedResponse<Payment>;
pub type CliListForwardedPaymentsResponse = CliPaginatedResponse<ForwardedPayment>;

fn format_page_token(token: PageToken) -> String {
	format!("{}:{}", token.token, token.index)
}

/// A denomination-aware amount that stores its value internally in millisatoshis.
///
/// Accepts the following formats when parsed from a string:
/// - `<number>sat` or `<number>sats` — interpreted as satoshis
/// - `<number>msat` or `<number>msats` — interpreted as millisatoshis
///
/// Bare numbers without a suffix are rejected.
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct Amount {
	msats: u64,
}

impl Amount {
	/// Returns the value in millisatoshis.
	pub fn to_msat(self) -> u64 {
		self.msats
	}

	/// Returns the value in satoshis.
	///
	/// Returns an error string if the value is not evenly divisible by 1000.
	pub fn to_sat(self) -> Result<u64, String> {
		if self.msats % 1000 != 0 {
			Err(format!(
				"amount {}msats is not evenly divisible by 1000, cannot convert to whole satoshis",
				self.msats
			))
		} else {
			Ok(self.msats / 1000)
		}
	}
}

impl fmt::Display for Amount {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}msats", self.msats)
	}
}

impl FromStr for Amount {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let s = s.trim();
		if let Some(num_str) = s.strip_suffix("msats") {
			let val: u64 = num_str
				.parse()
				.map_err(|_| format!("invalid amount: '{s}' — expected a number before 'msats'"))?;
			Ok(Amount { msats: val })
		} else if let Some(num_str) = s.strip_suffix("msat") {
			let val: u64 = num_str
				.parse()
				.map_err(|_| format!("invalid amount: '{s}' — expected a number before 'msat'"))?;
			Ok(Amount { msats: val })
		} else if let Some(num_str) = s.strip_suffix("sats") {
			let val: u64 = num_str
				.parse()
				.map_err(|_| format!("invalid amount: '{s}' — expected a number before 'sats'"))?;
			Ok(Amount {
				msats: val.checked_mul(1000).ok_or_else(|| "amount overflow".to_string())?,
			})
		} else if let Some(num_str) = s.strip_suffix("sat") {
			let val: u64 = num_str
				.parse()
				.map_err(|_| format!("invalid amount: '{s}' — expected a number before 'sat'"))?;
			Ok(Amount {
				msats: val.checked_mul(1000).ok_or_else(|| "amount overflow".to_string())?,
			})
		} else {
			Err(format!(
				"invalid amount: '{s}' — must include a denomination suffix (e.g. 1000sat, 5000msat)"
			))
		}
	}
}

/// An exact on-chain amount or all available on-chain funds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmountOrAll {
	Exact(Amount),
	All,
}

impl AmountOrAll {
	/// Returns the exact amount in satoshis, or `None` when all funds should be used.
	pub fn to_sat(self) -> Result<Option<u64>, String> {
		match self {
			Self::Exact(amount) => amount.to_sat().map(Some),
			Self::All => Ok(None),
		}
	}
}

impl FromStr for AmountOrAll {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if s.trim() == "all" {
			Ok(Self::All)
		} else {
			Amount::from_str(s).map(Self::Exact)
		}
	}
}

/// A validated 32-byte payment preimage, parsed from a 64-character hex string.
#[derive(Debug, Clone)]
pub struct Preimage(pub [u8; 32]);

impl Preimage {
	pub fn to_hex_string(&self) -> String {
		self.0.to_lower_hex_string()
	}
}

impl FromStr for Preimage {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		<[u8; 32]>::from_hex(s)
			.map(Preimage)
			.map_err(|_| "must be a 64-character hex string (32 bytes)".to_string())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn amount_parsing_and_conversion() {
		// sat suffix
		let amount = Amount::from_str("1000sat").unwrap();
		assert_eq!(amount.to_msat(), 1_000_000);
		assert_eq!(amount.to_sat().unwrap(), 1000);

		// sats suffix
		let amount = Amount::from_str("50sats").unwrap();
		assert_eq!(amount.to_msat(), 50_000);
		assert_eq!(amount.to_sat().unwrap(), 50);

		// msat suffix
		let amount = Amount::from_str("5000msat").unwrap();
		assert_eq!(amount.to_msat(), 5000);
		assert_eq!(amount.to_sat().unwrap(), 5);

		// msats suffix
		let amount = Amount::from_str("3000msats").unwrap();
		assert_eq!(amount.to_msat(), 3000);
		assert_eq!(amount.to_sat().unwrap(), 3);

		// zero
		let amount = Amount::from_str("0sat").unwrap();
		assert_eq!(amount.to_msat(), 0);
		assert_eq!(amount.to_sat().unwrap(), 0);
		let amount = Amount::from_str("0msat").unwrap();
		assert_eq!(amount.to_msat(), 0);
		assert_eq!(amount.to_sat().unwrap(), 0);

		// sat/msat equivalence
		let from_sat = Amount::from_str("5sat").unwrap();
		let from_msat = Amount::from_str("5000msat").unwrap();
		assert_eq!(from_sat.to_msat(), from_msat.to_msat());

		// to_sat rejects non-divisible msat values
		let amount = Amount::from_str("1500msat").unwrap();
		assert_eq!(amount.to_msat(), 1500);
		assert!(amount.to_sat().is_err());

		// rejects bare number
		assert!(Amount::from_str("1000").is_err());

		// rejects empty string
		assert!(Amount::from_str("").is_err());

		// rejects suffix with no number
		assert!(Amount::from_str("sat").is_err());
		assert!(Amount::from_str("msat").is_err());

		// rejects negative
		assert!(Amount::from_str("-100sat").is_err());
		assert!(Amount::from_str("-100msat").is_err());

		// rejects decimal
		assert!(Amount::from_str("1.5sat").is_err());
		assert!(Amount::from_str("1.5msat").is_err());

		// rejects overflow (u64::MAX sats would overflow when multiplied by 1000)
		let big = format!("{}sat", u64::MAX);
		assert!(Amount::from_str(&big).is_err());
	}

	#[test]
	fn amount_or_all_parses_exact_amount_or_all() {
		assert_eq!(AmountOrAll::from_str("all").unwrap(), AmountOrAll::All);
		assert_eq!(AmountOrAll::from_str(" all ").unwrap(), AmountOrAll::All);
		assert_eq!(AmountOrAll::from_str("100sat").unwrap().to_sat().unwrap(), Some(100));
		assert_eq!(AmountOrAll::All.to_sat().unwrap(), None);
	}

	#[test]
	fn preimage_parsing_and_roundtrip() {
		// valid 64-char hex string
		let hex = "2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b";
		let preimage = Preimage::from_str(hex).unwrap();
		assert_eq!(preimage.0, [0x2b; 32]);
		assert_eq!(preimage.to_hex_string(), hex);

		// rejects empty string
		assert!(Preimage::from_str("").is_err());

		// rejects too short (62 chars)
		assert!(Preimage::from_str(&"ab".repeat(31)).is_err());

		// rejects too long (66 chars)
		assert!(Preimage::from_str(&"ab".repeat(33)).is_err());

		// rejects non-hex characters
		assert!(Preimage::from_str(&"zz".repeat(32)).is_err());
	}
}
