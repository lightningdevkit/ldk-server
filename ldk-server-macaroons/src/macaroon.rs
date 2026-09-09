// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Macaroon v2 binary encoding and first-party HMAC chaining.
//!
//! Signatures use HMAC-SHA256 with constant-time verification.
//! Third-party caveats and other formats are rejected.
//! Format: <https://github.com/go-macaroon/macaroon/blob/v2/marshal-v2.go>.

use hex_conservative::{DisplayHex, FromHex};
use ring::{digest, hmac};

/// Maximum binary token size. Hex transport uses twice this many bytes.
pub const MAX_MACAROON_BYTES: usize = 4096;
/// Maximum number of first-party caveats.
pub const MAX_CAVEATS: usize = 32;
const KEY_GENERATOR: &[u8] = b"macaroons-key-generator";

/// Maximum difference between a request timestamp and server time, in seconds.
pub const REQUEST_TIMESTAMP_TOLERANCE_SECS: u64 = 60;
/// Reserved final caveat for a single RPC invocation. It is not a delegation restriction.
pub const REQUEST_CAVEAT_PREFIX: &str = "request = ";
const MAX_REQUEST_METHOD_BYTES: usize = 128;
// Prefix, u64 timestamp, separators, method, and lowercase SHA-256 digest.
const MAX_REQUEST_CAVEAT_BYTES: usize =
	REQUEST_CAVEAT_PREFIX.len() + 20 + 1 + MAX_REQUEST_METHOD_BYTES + 1 + 64;

/// A request proof carried by the final first-party caveat.
/// The body digest covers the exact gRPC body bytes, including the five-byte frame header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestBinding {
	/// Unix time in seconds.
	pub timestamp: u64,
	/// Short RPC method name, such as `GetNodeInfo`.
	pub method: String,
	/// SHA-256 hash of the exact gRPC body, including the frame header.
	pub body_sha256: [u8; 32],
}

impl RequestBinding {
	/// Bind a method and the exact request body to a Unix timestamp in seconds.
	pub fn new(method: &str, body: &[u8], timestamp: u64) -> Self {
		Self {
			timestamp,
			method: method.into(),
			body_sha256: digest::digest(&digest::SHA256, body).as_ref().try_into().unwrap(),
		}
	}

	/// Check the hash of the exact request body, including its gRPC frame header.
	pub fn matches_body(&self, body: &[u8]) -> bool {
		self.body_sha256.as_slice() == digest::digest(&digest::SHA256, body).as_ref()
	}

	/// Encode the canonical request caveat.
	pub fn caveat(&self) -> Result<String, &'static str> {
		use std::fmt::Write;
		if !valid_request_method(&self.method) {
			return Err("Invalid request method");
		}
		let mut caveat = format!("{REQUEST_CAVEAT_PREFIX}{} {} ", self.timestamp, self.method);
		for byte in self.body_sha256 {
			write!(caveat, "{byte:02x}").unwrap();
		}
		Ok(caveat)
	}

	/// Parse exactly one timestamp, method name, and lowercase SHA-256 digest.
	pub fn parse(caveat: &[u8]) -> Result<Self, &'static str> {
		let invalid = "Invalid request binding caveat";
		let caveat = std::str::from_utf8(caveat).map_err(|_| invalid)?;
		let mut fields = caveat.strip_prefix(REQUEST_CAVEAT_PREFIX).ok_or(invalid)?.split(' ');
		let timestamp_text = fields.next().ok_or(invalid)?;
		let timestamp: u64 = timestamp_text.parse().map_err(|_| invalid)?;
		let method = fields.next().ok_or(invalid)?;
		let digest = fields.next().ok_or(invalid)?;
		if timestamp_text != timestamp.to_string()
			|| !valid_request_method(method)
			|| digest.len() != 64
			|| fields.next().is_some()
			|| !digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
		{
			return Err(invalid);
		}
		let mut body_sha256 = [0; 32];
		for (index, byte) in body_sha256.iter_mut().enumerate() {
			*byte =
				u8::from_str_radix(&digest[index * 2..index * 2 + 2], 16).map_err(|_| invalid)?;
		}
		Ok(Self { timestamp, method: method.to_string(), body_sha256 })
	}
}

fn valid_request_method(method: &str) -> bool {
	!method.is_empty()
		&& method.len() <= MAX_REQUEST_METHOD_BYTES
		&& method.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// A parsed macaroon. Parsing alone does not authenticate it or validate its caveats.
#[derive(Clone)]
pub struct Macaroon {
	location: Option<Vec<u8>>,
	identifier: Vec<u8>,
	caveats: Vec<Vec<u8>>,
	signature: [u8; 32],
}

impl Macaroon {
	/// Parse a hex token. This does not verify its signature or caveats.
	pub fn from_hex(token: &str) -> Result<Self, &'static str> {
		if token.len() > MAX_MACAROON_BYTES * 2 {
			return Err("Macaroon exceeds size limit");
		}
		let data = Vec::<u8>::from_hex(token).map_err(|_| "Macaroon must be hexadecimal")?;
		Self::deserialize(&data)
	}

	/// Encode this token as lowercase hex.
	pub fn to_hex(&self) -> String {
		self.serialize().to_lower_hex_string()
	}

	/// Mint a macaroon using the standard root-key derivation and HMAC-SHA256.
	pub fn mint(root_key: &[u8], identifier: &[u8]) -> Result<Self, &'static str> {
		if identifier.is_empty() || identifier.len() > MAX_MACAROON_BYTES {
			return Err("Invalid macaroon identifier size");
		}
		let key = sign(KEY_GENERATOR, root_key);
		let macaroon = Self {
			location: None,
			identifier: identifier.to_vec(),
			caveats: Vec::new(),
			signature: sign(&key, identifier),
		};
		macaroon.check_size()?;
		Ok(macaroon)
	}

	/// The untrusted identifier used to select a server-side root key.
	pub fn identifier(&self) -> &[u8] {
		&self.identifier
	}

	/// Conditions that must all pass after signature verification.
	pub fn caveats(&self) -> &[Vec<u8>] {
		&self.caveats
	}

	/// Whether any caveat uses the reserved request-proof prefix.
	/// This checks presence only, not the proof's format, position, signature, or freshness.
	pub fn has_request_proof(&self) -> bool {
		self.caveats.iter().any(|c| c.starts_with(REQUEST_CAVEAT_PREFIX.as_bytes()))
	}

	/// Add a restriction without the root key. This cannot remove existing restrictions.
	pub fn attenuate(&mut self, caveat: &[u8]) -> Result<(), &'static str> {
		if caveat.is_empty()
			|| caveat.len() > MAX_MACAROON_BYTES
			|| self.caveats.len() >= MAX_CAVEATS
		{
			return Err("Invalid macaroon caveat size or count");
		}
		self.caveats.push(caveat.to_vec());
		if let Err(error) = self.check_size() {
			self.caveats.pop();
			return Err(error);
		}
		self.signature = sign(&self.signature, caveat);
		Ok(())
	}

	/// Ensure a reusable credential has room for a request proof of any supported length.
	pub fn check_request_capacity(&self) -> Result<(), &'static str> {
		// One field tag, two length bytes, and one end-of-caveat marker.
		if self.caveats.len() >= MAX_CAVEATS
			|| self.serialize().len() + MAX_REQUEST_CAVEAT_BYTES + 4 > MAX_MACAROON_BYTES
		{
			return Err("Macaroon has no room for a request binding caveat");
		}
		Ok(())
	}

	/// Verify the signature using HMAC-SHA256 and a constant-time HMAC verifier.
	/// This does not check caveat conditions: the caller must enforce every condition.
	pub fn verify_signature(&self, root_key: &[u8]) -> bool {
		let key = sign(KEY_GENERATOR, root_key);
		let Some((last, preceding)) = self.caveats.split_last() else {
			return verify(&key, &self.identifier, &self.signature);
		};
		let mut signature = sign(&key, &self.identifier);
		for caveat in preceding {
			signature = sign(&signature, caveat);
		}
		verify(&signature, last, &self.signature)
	}

	/// Serialize in standard v2 binary format.
	pub fn serialize(&self) -> Vec<u8> {
		let mut out = vec![2];
		if let Some(location) = &self.location {
			packet(&mut out, 1, location);
		}
		packet(&mut out, 2, &self.identifier);
		out.push(0);
		for caveat in &self.caveats {
			packet(&mut out, 2, caveat);
			out.push(0);
		}
		out.push(0);
		packet(&mut out, 6, &self.signature);
		out
	}

	fn check_size(&self) -> Result<(), &'static str> {
		if self.identifier.is_empty()
			|| self.identifier.len() > MAX_MACAROON_BYTES
			|| self.serialize().len() > MAX_MACAROON_BYTES
		{
			return Err("Invalid macaroon size");
		}
		Ok(())
	}

	/// Parse one bounded v2 token. Reject unknown fields, third-party caveats and trailing bytes.
	pub fn deserialize(mut data: &[u8]) -> Result<Self, &'static str> {
		if data.len() > MAX_MACAROON_BYTES || data.first() != Some(&2) {
			return Err("Invalid macaroon size or version");
		}
		data = &data[1..];
		let mut location = None;
		let (mut kind, mut value) = read_packet(&mut data)?;
		if kind == 1 {
			location = Some(value.to_vec());
			(kind, value) = read_packet(&mut data)?;
		}
		if kind != 2 || value.is_empty() {
			return Err("Invalid macaroon identifier");
		}
		let identifier = value.to_vec();
		if read_packet(&mut data)?.0 != 0 {
			return Err("Invalid macaroon header");
		}
		let mut caveats = Vec::new();
		loop {
			let (kind, value) = read_packet(&mut data)?;
			if kind == 0 {
				break;
			}
			if kind != 2 || value.is_empty() || caveats.len() >= MAX_CAVEATS {
				return Err("Unsupported or invalid macaroon caveat");
			}
			caveats.push(value.to_vec());
			if read_packet(&mut data)?.0 != 0 {
				return Err("Unsupported macaroon caveat fields");
			}
		}
		let (kind, value) = read_packet(&mut data)?;
		if kind != 6 || !data.is_empty() {
			return Err("Invalid macaroon signature field");
		}
		let signature = value.try_into().map_err(|_| "Invalid macaroon signature size")?;
		Ok(Self { location, identifier, caveats, signature })
	}
}

fn sign(key: &[u8], data: &[u8]) -> [u8; 32] {
	hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, key), data).as_ref().try_into().unwrap()
}

fn verify(key: &[u8], data: &[u8], signature: &[u8; 32]) -> bool {
	hmac::verify(&hmac::Key::new(hmac::HMAC_SHA256, key), data, signature).is_ok()
}

fn packet(out: &mut Vec<u8>, kind: u8, value: &[u8]) {
	out.push(kind);
	let mut length = value.len();
	while length >= 128 {
		out.push((length as u8 & 127) | 128);
		length >>= 7;
	}
	out.push(length as u8);
	out.extend_from_slice(value);
}

fn varint(data: &mut &[u8]) -> Result<usize, &'static str> {
	let mut value = 0usize;
	for shift in (0..35).step_by(7) {
		let (&byte, rest) = data.split_first().ok_or("Truncated macaroon field")?;
		*data = rest;
		if shift == 28 && byte > 7 {
			return Err("Macaroon varint overflow");
		}
		value |= ((byte & 127) as usize) << shift;
		if byte & 128 == 0 {
			if shift != 0 && byte == 0 {
				return Err("Noncanonical macaroon varint");
			}
			return Ok(value);
		}
	}
	Err("Macaroon varint overflow")
}

fn read_packet<'a>(data: &mut &'a [u8]) -> Result<(usize, &'a [u8]), &'static str> {
	let kind = varint(data)?;
	if kind == 0 {
		return Ok((0, &[]));
	}
	let length = varint(data)?;
	let value = data.get(..length).ok_or("Truncated macaroon payload")?;
	*data = &data[length..];
	Ok((kind, value))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn decode(hex: &str) -> Vec<u8> {
		hex.as_bytes()
			.chunks_exact(2)
			.map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
			.collect()
	}

	#[test]
	fn request_binding_encoding_is_canonical_and_bounded() {
		let binding = RequestBinding {
			timestamp: 123,
			method: "GetNodeInfo".into(),
			body_sha256: [0xab; 32],
		};
		let expected = format!("request = 123 GetNodeInfo {}", "ab".repeat(32));
		assert_eq!(binding.caveat().unwrap(), expected);
		assert_eq!(RequestBinding::parse(expected.as_bytes()).unwrap(), binding);
		for invalid in [
			expected.replace("123", "0123"),
			expected.replace("123", "+123"),
			expected.replace("123", "18446744073709551616"),
			expected.replace("123", "-1"),
			expected.replace("123 ", "123  "),
			expected.replace("GetNodeInfo", ""),
			expected.replace("GetNodeInfo", "/api.LightningNode/GetNodeInfo"),
			expected.replace("GetNodeInfo", &"X".repeat(MAX_REQUEST_METHOD_BYTES + 1)),
			expected.replace("ab", "AB"),
			expected.replace("ab", "zz"),
			format!("{expected} extra"),
			format!("{expected}\n"),
			expected[..expected.len() - 1].into(),
			expected.replace("request = ", "body = "),
		] {
			assert!(RequestBinding::parse(invalid.as_bytes()).is_err(), "{invalid}");
		}
		assert!(RequestBinding::parse(&[255]).is_err());
		let maximum = RequestBinding {
			timestamp: u64::MAX,
			method: "X".repeat(MAX_REQUEST_METHOD_BYTES),
			body_sha256: [255; 32],
		};
		assert_eq!(maximum.caveat().unwrap().len(), MAX_REQUEST_CAVEAT_BYTES);
		assert_eq!(RequestBinding::parse(maximum.caveat().unwrap().as_bytes()).unwrap(), maximum);
	}

	#[test]
	fn reference_tokens_roundtrip_and_reject_truncation() {
		for line in
			include_str!("../tests/data/macaroons-v2.txt").lines().filter(|l| !l.starts_with('#'))
		{
			let fields: Vec<_> = line.split_whitespace().collect();
			let bytes = decode(fields[2]);
			let macaroon = Macaroon::deserialize(&bytes).unwrap();
			assert_eq!(macaroon.identifier(), decode(fields[1]));
			assert_eq!(macaroon.serialize(), bytes);
			for end in 0..bytes.len() {
				assert!(Macaroon::deserialize(&bytes[..end]).is_err());
			}
			let mut trailing = bytes.clone();
			trailing.push(0);
			assert!(Macaroon::deserialize(&trailing).is_err());
		}
	}

	#[test]
	fn mutated_reference_tokens_never_panic_and_accepted_tokens_roundtrip() {
		for line in
			include_str!("../tests/data/macaroons-v2.txt").lines().filter(|l| !l.starts_with('#'))
		{
			let bytes = decode(line.split_whitespace().nth(2).unwrap());
			for index in 0..bytes.len() {
				for value in 0..=255 {
					let mut mutated = bytes.clone();
					mutated[index] = value;
					if let Ok(parsed) = Macaroon::deserialize(&mutated) {
						assert_eq!(parsed.serialize(), mutated);
					}
				}
			}
		}
	}

	#[test]
	fn rejects_unsupported_fields_and_bad_lengths() {
		let header = [2, 2, 1, b'i', 0];
		for body in [
			vec![2, 1, b'c', 4, 1, b'v', 0], // Third-party verification identifier.
			vec![1, 1, b'l', 2, 1, b'c', 0], // Third-party location.
			vec![3, 1, b'x', 0],             // Unknown field.
			vec![2, 1, b'c', 2, 1, b'd', 0], // Duplicate identifier.
			vec![2, 0, 0],                   // Empty caveat.
			vec![2, 255, 255, 255, 255, 127], // Overflow.
			vec![2, 128, 0],                 // Noncanonical length.
		] {
			let mut bytes = header.to_vec();
			bytes.extend(body);
			bytes.extend([0, 6, 32]);
			bytes.extend([0; 32]);
			assert!(Macaroon::deserialize(&bytes).is_err());
		}
		assert!(Macaroon::deserialize(&vec![2; MAX_MACAROON_BYTES + 1]).is_err());
		let mut bytes = header.to_vec();
		for _ in 0..MAX_CAVEATS + 1 {
			bytes.extend([2, 1, b'c', 0]);
		}
		bytes.extend([0, 6, 32]);
		bytes.extend([0; 32]);
		assert!(Macaroon::deserialize(&bytes).is_err());
	}

	#[test]
	fn standard_signatures_match_reference_implementation() {
		for (index, line) in include_str!("../tests/data/macaroons-v2.txt")
			.lines()
			.filter(|line| !line.starts_with('#'))
			.enumerate()
		{
			let fields: Vec<_> = line.split_whitespace().collect();
			let root = Vec::<u8>::from_hex(fields[0]).unwrap();
			let id = Vec::<u8>::from_hex(fields[1]).unwrap();
			let mut bytes = Vec::<u8>::from_hex(fields[2]).unwrap();
			let m = Macaroon::deserialize(&bytes).unwrap();
			assert!(m.verify_signature(&root));
			assert!(!m.verify_signature(b"incorrect root"));
			let mut issued = Macaroon::mint(&root, &id).unwrap();
			for caveat in m.caveats() {
				issued.attenuate(caveat).unwrap();
			}
			if index != 3 {
				// The reference writes an empty location; our issuer omits this optional hint.
				let mut without_empty_location = bytes.clone();
				assert_eq!(&without_empty_location[1..3], &[1, 0]);
				without_empty_location.drain(1..3);
				assert_eq!(issued.serialize(), without_empty_location);
			} // Case 3 has a location hint.
			*bytes.last_mut().unwrap() ^= 1;
			assert!(!Macaroon::deserialize(&bytes).unwrap().verify_signature(&root));
		}
	}

	#[test]
	fn attenuation_limits_leave_token_unchanged() {
		let mut token = Macaroon::mint(b"key", b"id").unwrap();
		for _ in 0..MAX_CAVEATS {
			token.attenuate(b"permissions = admin").unwrap();
		}
		let original = token.serialize();
		assert!(token.attenuate(b"permissions = admin").is_err());
		assert_eq!(token.serialize(), original);
		let mut token = Macaroon::mint(b"key", b"id").unwrap();
		let original = token.serialize();
		assert!(token.attenuate(&vec![b'x'; MAX_MACAROON_BYTES]).is_err());
		assert_eq!(token.serialize(), original);
	}
}
