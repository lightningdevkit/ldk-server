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
//! Callers supply HMAC-SHA256 from their existing cryptographic library. Verification must
//! use a constant-time HMAC verifier. Third-party caveats and other formats are rejected.
//! Format: <https://github.com/go-macaroon/macaroon/blob/v2/marshal-v2.go>.

/// Maximum binary token size. Hex transport uses twice this many bytes.
pub const MAX_MACAROON_BYTES: usize = 4096;
/// Maximum number of first-party caveats.
pub const MAX_CAVEATS: usize = 32;
const KEY_GENERATOR: &[u8] = b"macaroons-key-generator";

/// A parsed macaroon. Parsing alone does not authenticate it or validate its caveats.
#[derive(Clone)]
pub struct Macaroon {
	location: Option<Vec<u8>>,
	identifier: Vec<u8>,
	caveats: Vec<Vec<u8>>,
	signature: [u8; 32],
}

impl Macaroon {
	/// Mint a macaroon using the standard root-key derivation and HMAC-SHA256.
	pub fn mint(
		root_key: &[u8], identifier: &[u8], hmac: impl Fn(&[u8], &[u8]) -> [u8; 32],
	) -> Result<Self, &'static str> {
		if identifier.is_empty() || identifier.len() > MAX_MACAROON_BYTES {
			return Err("Invalid macaroon identifier size");
		}
		let key = hmac(KEY_GENERATOR, root_key);
		let macaroon = Self {
			location: None,
			identifier: identifier.to_vec(),
			caveats: Vec::new(),
			signature: hmac(&key, identifier),
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

	/// Add a restriction without the root key. This cannot remove existing restrictions.
	pub fn attenuate(
		&mut self, caveat: &[u8], hmac: impl Fn(&[u8], &[u8]) -> [u8; 32],
	) -> Result<(), &'static str> {
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
		self.signature = hmac(&self.signature, caveat);
		Ok(())
	}

	/// Verify the signature using HMAC-SHA256 and a constant-time HMAC verifier.
	/// This does not check caveat conditions: the caller must enforce every condition.
	pub fn verify_signature(
		&self, root_key: &[u8], hmac: impl Fn(&[u8], &[u8]) -> [u8; 32],
		verify: impl Fn(&[u8], &[u8], &[u8; 32]) -> bool,
	) -> bool {
		let key = hmac(KEY_GENERATOR, root_key);
		let Some((last, preceding)) = self.caveats.split_last() else {
			return verify(&key, &self.identifier, &self.signature);
		};
		let mut signature = hmac(&key, &self.identifier);
		for caveat in preceding {
			signature = hmac(&signature, caveat);
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
}
