// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Derive restricted macaroons and bind them to requests without contacting the server.

use crate::{Macaroon, RequestBinding, REQUEST_CAVEAT_PREFIX};

/// Parse a private reusable credential and check that it has room for request binding.
/// This rejects request tokens but does not verify the signature or enforce caveats.
pub fn parse_reusable_macaroon(token: &str) -> Result<Macaroon, String> {
	let macaroon = Macaroon::from_hex(token).map_err(str::to_string)?;
	if macaroon.has_request_proof() {
		return Err("A request-bound macaroon cannot be used as a reusable credential".into());
	}
	macaroon.check_request_capacity().map_err(str::to_string)?;
	Ok(macaroon)
}

/// Derive a restricted copy of a hex-encoded v2 macaroon. Keep both tokens private.
///
/// Caveats can limit permissions (`permissions = node:read,payments:read`),
/// the RPC method (`method = GetNodeInfo`), or expiry (`time-before = 1800000000`).
/// Expiry is a Unix time in seconds. All caveats must pass; added caveats can only reduce access.
/// This function can encode unknown conditions, but the server rejects them.
pub fn derive_macaroon(token: &str, caveats: &[String]) -> Result<String, String> {
	let mut macaroon = parse_reusable_macaroon(token)?;
	for caveat in caveats {
		if caveat.starts_with(REQUEST_CAVEAT_PREFIX) {
			return Err("Use bind_macaroon_to_request to create a request proof".into());
		}
		macaroon.attenuate(caveat.as_bytes()).map_err(str::to_string)?;
	}
	macaroon.check_request_capacity().map_err(str::to_string)?;
	Ok(macaroon.to_hex())
}

/// Make a token tied to an RPC method, body, and the current time.
///
/// Keep the original macaroon private. Send the returned token in the `macaroon` header.
/// Use a method name such as `GetNodeInfo`. `body` must include the exact gRPC bytes sent,
/// including the five-byte frame header. Clocks must be within 60 seconds.
/// The same request can still be replayed while the token is valid.
pub fn bind_macaroon_to_request(token: &str, method: &str, body: &[u8]) -> Result<String, String> {
	let timestamp = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map_err(|_| "System time is before the Unix epoch")?
		.as_secs();
	bind_macaroon_to_request_at(token, method, body, timestamp)
}

/// Bind a request with an explicit Unix timestamp in seconds.
///
/// Use this with a custom clock or in tests. The same token, method, and body rules as
/// [`bind_macaroon_to_request`] apply. The server still requires a fresh timestamp.
pub fn bind_macaroon_to_request_at(
	token: &str, method: &str, body: &[u8], timestamp: u64,
) -> Result<String, String> {
	let binding = RequestBinding::new(method, body, timestamp);
	let mut macaroon = parse_reusable_macaroon(token)?;
	macaroon.attenuate(binding.caveat()?.as_bytes()).map_err(str::to_string)?;
	Ok(macaroon.to_hex())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::MAX_MACAROON_BYTES;

	#[test]
	fn request_proof_matches_external_reference() {
		let rows: Vec<_> = include_str!("../tests/data/macaroons-v2.txt")
			.lines()
			.filter(|line| !line.starts_with('#'))
			.collect();
		let credential = rows[1].split_whitespace().nth(2).unwrap();
		let expected = rows.last().unwrap().split_whitespace().nth(2).unwrap();
		assert_eq!(
			bind_macaroon_to_request_at(credential, "GetNodeInfo", &[0; 5], 1800000000).unwrap(),
			expected
		);
	}

	#[test]
	fn request_binding_preserves_private_credential_and_signs_exact_body() {
		let root = b"test root";
		let mut credential = Macaroon::mint(root, b"test id").unwrap();
		credential.attenuate(b"permissions = node:read").unwrap();
		let token = credential.to_hex();
		let body = b"\0\0\0\0\x03abc";
		let header = bind_macaroon_to_request(&token, "GetNodeInfo", body).unwrap();
		assert_ne!(header, token);
		assert_eq!(parse_reusable_macaroon(&token).unwrap().caveats().len(), 1);
		let transmitted = Macaroon::from_hex(&header).unwrap();
		assert_eq!(&transmitted.caveats()[..1], credential.caveats());
		let binding = RequestBinding::parse(transmitted.caveats().last().unwrap()).unwrap();
		assert_eq!(binding.method, "GetNodeInfo");
		assert!(binding.matches_body(body));
		assert!(!binding.matches_body(b"changed"));
		assert!(transmitted.verify_signature(root));
		assert!(parse_reusable_macaroon(&header).is_err());
		assert!(bind_macaroon_to_request(&header, "GetBalances", b"changed").is_err());
		assert!(derive_macaroon(&token, &[binding.caveat().unwrap()]).is_err());
	}

	#[test]
	fn offline_restrictions_reserve_space_for_request_binding() {
		let token = Macaroon::mint(b"root", b"id").unwrap().to_hex();
		let fits = vec!["permissions = node:read".into(); crate::MAX_CAVEATS - 1];
		let fits = derive_macaroon(&token, &fits).unwrap();
		assert!(bind_macaroon_to_request(&fits, "GetNodeInfo", b"").is_ok());
		assert!(derive_macaroon(&fits, &["method = GetNodeInfo".into()]).is_err());
		assert!(derive_macaroon(&token, &["x".repeat(MAX_MACAROON_BYTES - 100)]).is_err());
	}

	#[test]
	fn request_capacity_accepts_exact_limit_and_rejects_one_more_byte() {
		let root = b"root";
		let credential = Macaroon::mint(root, b"id").unwrap();
		let method = "X".repeat(128);
		let body = [0; 5];
		let proof = RequestBinding::new(&method, &body, u64::MAX).caveat().unwrap();
		// Each long caveat has a tag, two length bytes, and an end marker.
		let padding_len = MAX_MACAROON_BYTES - credential.serialize().len() - proof.len() - 8;
		let padding = "x".repeat(padding_len);
		let token = derive_macaroon(&credential.to_hex(), &[padding]).unwrap();
		parse_reusable_macaroon(&token).unwrap().check_request_capacity().unwrap();
		let bound = bind_macaroon_to_request_at(&token, &method, &body, u64::MAX).unwrap();
		let bound = Macaroon::from_hex(&bound).unwrap();
		assert_eq!(bound.serialize().len(), MAX_MACAROON_BYTES);
		assert!(bound.verify_signature(root));
		assert_eq!(bound.caveats().last().unwrap(), proof.as_bytes());

		let mut too_large = credential.clone();
		let padding = "x".repeat(padding_len + 1);
		too_large.attenuate(padding.as_bytes()).unwrap();
		assert!(too_large.check_request_capacity().is_err());
		assert!(parse_reusable_macaroon(&too_large.to_hex()).is_err());
		assert!(derive_macaroon(&credential.to_hex(), &[padding]).is_err());
		assert!(bind_macaroon_to_request_at(&too_large.to_hex(), &method, &body, u64::MAX).is_err());
	}

	#[test]
	fn request_binding_rejects_invalid_method_names() {
		let token = Macaroon::mint(b"root", b"id").unwrap().to_hex();
		for method in [
			"",
			"Get_NodeInfo",
			"Get-NodeInfo",
			"Get NodeInfo",
			"GetNodeInfo\n",
			"GétNodeInfo",
			"/api.LightningNode/GetNodeInfo",
			&"X".repeat(129),
		] {
			assert_eq!(
				RequestBinding::new(method, b"", 123).caveat(),
				Err("Invalid request method")
			);
			assert_eq!(
				bind_macaroon_to_request(&token, method, b""),
				Err("Invalid request method".into())
			);
		}
		for method in ["A", "Rpc123", &"X".repeat(128)] {
			let bound = bind_macaroon_to_request(&token, method, b"").unwrap();
			let bound = Macaroon::from_hex(&bound).unwrap();
			let proof = RequestBinding::parse(bound.caveats().last().unwrap()).unwrap();
			assert_eq!(proof.method, method);
		}
	}

	#[test]
	fn derivation_matches_reference_implementation() {
		let tokens: Vec<_> = include_str!("../tests/data/macaroons-v2.txt")
			.lines()
			.filter(|line| !line.starts_with('#'))
			.map(|line| line.split_whitespace().nth(2).unwrap())
			.collect();
		let first = derive_macaroon(tokens[0], &["permissions = node:read".into()]).unwrap();
		assert_eq!(first, tokens[1]);
		assert_eq!(derive_macaroon(&first, &["method = GetNodeInfo".into()]).unwrap(), tokens[2]);
		assert!(derive_macaroon("deadbeef", &[]).is_err());
		assert!(derive_macaroon(&"00".repeat(MAX_MACAROON_BYTES + 1), &[]).is_err());
	}
}
