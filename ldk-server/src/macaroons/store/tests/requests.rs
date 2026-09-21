// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use super::*;

#[test]
fn request_proofs_bind_method_body_identifier_and_timestamp() {
	let (_directory, store) = test_store("request-proof");
	let credential = admin_token(&store);
	let body = b"\x00\x00\x00\x00\x03abc";
	let timestamp = now();
	let header = bind_request(&credential, GET_NODE_INFO_PATH, body, timestamp);
	for _ in 0..2 {
		// Timestamp freshness deliberately permits identical replays within its window.
		let pending = store.authenticate_request(GET_NODE_INFO_PATH, Some(&header)).unwrap();
		assert!(store.finish_request(pending, GET_NODE_INFO_PATH, body).unwrap().is_admin());
	}
	assert!(store.authenticate_request(GET_BALANCES_PATH, Some(&header)).is_err());
	for changed in [b"\x00\x00\x00\x00\x03abd".as_slice(), b"abc", b""] {
		let pending = store.authenticate_request(GET_NODE_INFO_PATH, Some(&header)).unwrap();
		assert_eq!(
			store.finish_request(pending, GET_NODE_INFO_PATH, changed).unwrap_err().error_code,
			LdkServerErrorCode::AuthError
		);
	}
	for stale in [timestamp - 61, timestamp + 600] {
		let header = bind_request(&credential, GET_NODE_INFO_PATH, body, stale);
		assert!(store.authenticate_request(GET_NODE_INFO_PATH, Some(&header)).is_err());
	}
	let bytes = Vec::<u8>::from_hex(&header).unwrap();
	let parsed = Macaroon::deserialize(&bytes).unwrap();
	let identifier_offset =
		bytes.windows(parsed.identifier().len()).position(|w| w == parsed.identifier()).unwrap();
	let timestamp_bytes = timestamp.to_string();
	let timestamp_offset =
		bytes.windows(timestamp_bytes.len()).position(|w| w == timestamp_bytes.as_bytes()).unwrap();
	for offset in [identifier_offset, timestamp_offset, bytes.len() - 1] {
		let mut forged = bytes.clone();
		forged[offset] ^= 1;
		assert!(store
			.authenticate_request(GET_NODE_INFO_PATH, Some(&forged.to_lower_hex_string()))
			.is_err());
	}
}

#[test]
fn request_proof_is_required_once_and_cannot_be_extended_or_rebound() {
	let (_directory, store) = test_store("request-proof-required");
	let credential = admin_token(&store);
	assert!(store.authenticate_request(GET_NODE_INFO_PATH, Some(&credential)).is_err());
	let bound = bind_request(&credential, GET_NODE_INFO_PATH, b"body", now());
	let rebound = append_request_proof(&bound, GET_BALANCES_PATH, b"different", now());
	assert!(store.authenticate_request(GET_BALANCES_PATH, Some(&rebound)).is_err());
	let duplicate = append_request_proof(&bound, GET_NODE_INFO_PATH, b"body", now());
	assert!(store.authenticate_request(GET_NODE_INFO_PATH, Some(&duplicate)).is_err());
	let extended = restrict(&bound, &["permissions = node:read"]);
	assert!(store.authenticate_request(GET_NODE_INFO_PATH, Some(&extended)).is_err());
	for malformed in [
		"request = ",
		"request = 00 GetNodeInfo deadbeef",
		"request = 18446744073709551616 GetNodeInfo deadbeef",
	] {
		let header = restrict(&credential, &[malformed]);
		assert!(store.authenticate_request(GET_NODE_INFO_PATH, Some(&header)).is_err());
	}
	// A body hash and fresh timestamp do not remove the holder's policy restrictions.
	let restricted = restrict(&credential, &["method = GetNodeInfo"]);
	let bound = bind_request(&restricted, GET_BALANCES_PATH, b"", now());
	assert_eq!(
		store.authenticate_request(GET_BALANCES_PATH, Some(&bound)).unwrap_err().error_code,
		LdkServerErrorCode::AuthorizationError
	);
}

// Deliberately bypass reusable-token checks to test rejection of multiple proofs.
fn append_request_proof(token: &str, method: &str, body: &[u8], timestamp: u64) -> String {
	let proof = RequestBinding::new(method, body, timestamp);
	restrict(token, &[&proof.caveat().unwrap()])
}
