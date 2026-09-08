// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Offline restriction of macaroon credentials. No server connection or root key is needed.

use bitcoin_hashes::hmac::{Hmac, HmacEngine};
use bitcoin_hashes::{sha256, Hash, HashEngine};
use hex_conservative::{DisplayHex, FromHex};
use ldk_server_grpc::macaroon::{Macaroon, MAX_MACAROON_BYTES};

pub(crate) fn parse_macaroon(token: &str) -> Result<Macaroon, String> {
	if token.len() > MAX_MACAROON_BYTES * 2 {
		return Err("Macaroon exceeds size limit".into());
	}
	let data = Vec::<u8>::from_hex(token).map_err(|_| "Macaroon must be hexadecimal")?;
	Macaroon::deserialize(&data).map_err(str::to_string)
}

/// Add caveats to a hex-encoded v2 macaroon, without contacting the server.
///
/// Supported server conditions are `permissions = node:read,payments:read`,
/// `method = GetNodeInfo`, and `time-before = 1800000000` (exclusive Unix seconds).
/// Every caveat must pass; permission sets intersect and expiry can only become earlier.
/// Unknown conditions can be encoded but the server will deny them.
/// The returned token is a bearer credential and must be kept private.
pub fn attenuate_macaroon(token: &str, caveats: &[String]) -> Result<String, String> {
	let mut macaroon = parse_macaroon(token)?;
	for caveat in caveats {
		macaroon
			.attenuate(caveat.as_bytes(), |key, data| {
				let mut engine = HmacEngine::<sha256::Hash>::new(key);
				engine.input(data);
				Hmac::from_engine(engine).to_byte_array()
			})
			.map_err(str::to_string)?;
	}
	Ok(macaroon.serialize().to_lower_hex_string())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn attenuation_matches_reference_implementation() {
		let tokens: Vec<_> = include_str!("../../ldk-server-grpc/tests/data/macaroons-v2.txt")
			.lines()
			.filter(|line| !line.starts_with('#'))
			.map(|line| line.split_whitespace().nth(2).unwrap())
			.collect();
		let first = attenuate_macaroon(tokens[0], &["permissions = node:read".into()]).unwrap();
		assert_eq!(first, tokens[1]);
		assert_eq!(
			attenuate_macaroon(&first, &["method = GetNodeInfo".into()]).unwrap(),
			tokens[2]
		);
		assert!(attenuate_macaroon("deadbeef", &[]).is_err());
		assert!(attenuate_macaroon(&"00".repeat(MAX_MACAROON_BYTES + 1), &[]).is_err());
	}
}
