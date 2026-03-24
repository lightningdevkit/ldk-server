// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Custom serde serializers/deserializers for byte fields.
//!
//! These are used via `#[serde(with = "...")]` attributes on fields to produce
//! human-readable hex output for byte data.

use std::fmt::Write;

use serde::{Deserialize, Deserializer, Serializer};

/// Module for serializing/deserializing `Vec<u8>` as a hex string.
pub mod bytes_hex {
	use super::*;

	pub fn serialize<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let hex = value.iter().fold(String::with_capacity(value.len() * 2), |mut acc, b| {
			let _ = write!(acc, "{b:02x}");
			acc
		});
		serializer.serialize_str(&hex)
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
	where
		D: Deserializer<'de>,
	{
		let hex = String::deserialize(deserializer)?;
		hex_to_bytes(&hex).map_err(serde::de::Error::custom)
	}
}

/// Module for serializing/deserializing `[u8; 32]` as a hex string.
pub mod hex_32 {
	use super::*;

	pub fn serialize<S>(value: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let hex = value.iter().fold(String::with_capacity(64), |mut acc, b| {
			let _ = write!(acc, "{b:02x}");
			acc
		});
		serializer.serialize_str(&hex)
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
	where
		D: Deserializer<'de>,
	{
		let hex = String::deserialize(deserializer)?;
		hex_to_32_bytes(&hex).map_err(serde::de::Error::custom)
	}
}

/// Module for serializing/deserializing `Option<[u8; 32]>` as a hex string (or null).
pub mod opt_hex_32 {
	use super::*;

	pub fn serialize<S>(value: &Option<[u8; 32]>, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		match value {
			Some(bytes) => {
				let hex = bytes.iter().fold(String::with_capacity(64), |mut acc, b| {
					let _ = write!(acc, "{b:02x}");
					acc
				});
				serializer.serialize_some(&hex)
			},
			None => serializer.serialize_none(),
		}
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<[u8; 32]>, D::Error>
	where
		D: Deserializer<'de>,
	{
		let opt: Option<String> = Option::deserialize(deserializer)?;
		match opt {
			Some(hex) => {
				let bytes = hex_to_32_bytes(&hex).map_err(serde::de::Error::custom)?;
				Ok(Some(bytes))
			},
			None => Ok(None),
		}
	}
}

fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
	if hex.len() % 2 != 0 {
		return Err("Hex string must have even length".to_string());
	}
	(0..hex.len())
		.step_by(2)
		.map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string()))
		.collect()
}

/// Module for serializing/deserializing `[u8; 33]` as a hex string.
pub mod hex_33 {
	use super::*;

	pub fn serialize<S>(value: &[u8; 33], serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let hex = value.iter().fold(String::with_capacity(66), |mut acc, b| {
			let _ = write!(acc, "{b:02x}");
			acc
		});
		serializer.serialize_str(&hex)
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 33], D::Error>
	where
		D: Deserializer<'de>,
	{
		let hex = String::deserialize(deserializer)?;
		hex_to_fixed::<33>(&hex).map_err(serde::de::Error::custom)
	}
}

fn hex_to_fixed<const N: usize>(hex: &str) -> Result<[u8; N], String> {
	let bytes = hex_to_bytes(hex)?;
	bytes.try_into().map_err(|v: Vec<u8>| format!("expected {} bytes, got {}", N, v.len()))
}

fn hex_to_32_bytes(hex: &str) -> Result<[u8; 32], String> {
	hex_to_fixed::<32>(hex)
}
