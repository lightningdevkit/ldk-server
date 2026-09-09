// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use super::*;
use crate::macaroons::{method_authorization, MethodAuthorization};

#[test]
fn splicing_requires_its_own_permission() {
	let (directory, store) = test_store("splice-permission");
	let admin = store.list_roots().unwrap().remove(0);
	let manager = store
		.create_root("manager", vec![CHANNELS_MANAGE_PERMISSION.to_string()], &admin)
		.unwrap()
		.info;
	let splicer = store
		.create_root("splicer", vec![CHANNELS_SPLICE_PERMISSION.to_string()], &admin)
		.unwrap()
		.info;
	for method in [SPLICE_IN_PATH, SPLICE_OUT_PATH] {
		let MethodAuthorization::Permission(permission) = method_authorization(method) else {
			panic!("Splicing must require a permission");
		};
		assert!(!manager.allows(permission));
		assert!(splicer.allows(permission));
		assert!(admin.allows(permission));
	}
	assert!(store
		.create_root("delegated-splicer", vec![CHANNELS_SPLICE_PERMISSION.to_string()], &manager,)
		.is_err());
	let reloaded = MacaroonStore::load_or_create(&directory).unwrap();
	assert!(reloaded.list_roots().unwrap().contains(&splicer));
}

#[test]
fn attenuation_intersects_permissions_and_enforces_all_conditions() {
	let (_directory, store) = test_store("attenuation");
	let admin = admin_token(&store);
	let token = restrict(
		&admin,
		&[
			"permissions = node:read,payments:read",
			"permissions = node:read",
			"permissions = admin",
		],
	);
	let reader = store.authenticate(GET_NODE_INFO_PATH, Some(&token)).unwrap();
	assert_eq!(reader.permissions, BTreeSet::from([NODE_READ_PERMISSION.to_string()]));
	assert!(!reader.is_admin());
	assert!(store.create_root("escalated", vec![ADMIN_PERMISSION.into()], &reader).is_err());
	let disjoint = restrict(&token, &["permissions = invoices:create"]);
	assert!(store
		.authenticate(GET_NODE_INFO_PATH, Some(&disjoint))
		.unwrap()
		.permissions
		.is_empty());
	let method = restrict(&token, &["method = GetNodeInfo"]);
	assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&method)).is_ok());
	assert!(store.authenticate(GET_BALANCES_PATH, Some(&method)).is_err());
	let future = format!("time-before = {}", now() + 3600);
	assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&restrict(&token, &[&future]))).is_ok());
	for caveat in [
		"time-before = 0",
		"time-before = 00",
		"time-before = -1",
		"time-before = 18446744073709551616",
		"unknown = true",
		"permissions = unknown",
		"permissions = admin,node:read",
		"permissions = ",
	] {
		assert!(
			store.authenticate(GET_NODE_INFO_PATH, Some(&restrict(&token, &[caveat]))).is_err(),
			"{caveat}"
		);
	}
	let expired = restrict(&token, &["time-before = 0", &future]);
	assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&expired)).is_err());
}

#[test]
fn rejects_forgery_and_revokes_all_attenuated_copies() {
	let (directory, store) = test_store("revocation");
	let admin = store.list_roots().unwrap().remove(0);
	let reader = store.create_root("reader", vec![NODE_READ_PERMISSION.into()], &admin).unwrap();
	let token = restrict(&reader.token, &["method = GetNodeInfo"]);
	assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&token)).is_ok());
	// Change a caveat without recomputing the chain.
	let bytes = Vec::<u8>::from_hex(&token).unwrap();
	let mut modified = bytes.clone();
	let offset = modified.windows(11).position(|w| w == b"GetNodeInfo").unwrap();
	modified[offset] = b'X';
	assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&modified.to_lower_hex_string())).is_err());
	// Remove the last caveat while retaining the final signature.
	let mut removed = Vec::<u8>::from_hex(&reader.token).unwrap();
	let len = removed.len();
	removed[len - 32..].copy_from_slice(&bytes[bytes.len() - 32..]);
	assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&removed.to_lower_hex_string())).is_err());
	for header in [None, Some(""), Some("not-a-macaroon"), Some("deadbeef")] {
		assert_eq!(
			store.authenticate(GET_NODE_INFO_PATH, header).unwrap_err().error_code,
			LdkServerErrorCode::AuthError
		);
	}
	let root = { store.roots.read().unwrap().get(&reader.info.id).unwrap().secret.clone() };
	assert_ne!(reader.token, root);
	store.revoke_root(&reader.info.id, &admin).unwrap();
	for credential in [&reader.token, &token] {
		assert_eq!(
			store.authenticate(GET_NODE_INFO_PATH, Some(credential)).unwrap_err().error_code,
			LdkServerErrorCode::AuthError
		);
		assert!(MacaroonStore::load_or_create(&directory)
			.unwrap()
			.authenticate(GET_NODE_INFO_PATH, Some(credential))
			.is_err());
	}
	assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&admin_token(&store))).is_ok());
}

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

#[test]
fn request_proofs_do_not_become_child_restrictions() {
	let (_directory, store) = test_store("request-proof-delegation");
	let expiry = format!("time-before = {}", now() + 3600);
	let credential = restrict(&admin_token(&store), &[&expiry]);
	let bound = bind_request(&credential, CREATE_MACAROON_PATH, b"create-body", now());
	let pending = store.authenticate_request(CREATE_MACAROON_PATH, Some(&bound)).unwrap();
	let issuer = store.finish_request(pending, CREATE_MACAROON_PATH, b"create-body").unwrap();
	assert!(issuer.caveats.contains(&expiry));
	assert!(!issuer.caveats.iter().any(|c| c.starts_with("request = ")));
	let child = store.create_root("child", vec![ADMIN_PERMISSION.into()], &issuer).unwrap();
	let bound = bind_request(&child.token, CREATE_MACAROON_PATH, b"grandchild-body", now());
	let pending = store.authenticate_request(CREATE_MACAROON_PATH, Some(&bound)).unwrap();
	let child_issuer =
		store.finish_request(pending, CREATE_MACAROON_PATH, b"grandchild-body").unwrap();
	let grandchild =
		store.create_root("grandchild", vec![NODE_READ_PERMISSION.into()], &child_issuer).unwrap();
	assert!(grandchild.info.caveats.contains(&expiry));
	let bound = bind_request(&grandchild.token, GET_PERMISSIONS_PATH, b"", now());
	let pending = store.authenticate_request(GET_PERMISSIONS_PATH, Some(&bound)).unwrap();
	let info = store.finish_request(pending, GET_PERMISSIONS_PATH, b"").unwrap();
	assert!(!info.caveats.iter().any(|c| c.starts_with("request = ")));
	assert!(!info.caveats.iter().any(|c| c == "method = CreateMacaroon"));
}

#[test]
fn revocation_and_freshness_are_rechecked_after_reading_the_body() {
	let (_directory, store) = test_store("request-proof-finish");
	let admin = store.authenticate(CREATE_MACAROON_PATH, Some(&admin_token(&store))).unwrap();
	let child = store.create_root("child", vec![NODE_READ_PERMISSION.into()], &admin).unwrap();
	let bound = bind_request(&child.token, GET_NODE_INFO_PATH, b"", now());
	let pending = store.authenticate_request(GET_NODE_INFO_PATH, Some(&bound)).unwrap();
	store.revoke_root(&child.info.id, &admin).unwrap();
	assert!(store.finish_request(pending, GET_NODE_INFO_PATH, b"").is_err());
	let bound = bind_request(&admin_token(&store), GET_NODE_INFO_PATH, b"", now());
	let pending = store.authenticate_request(GET_NODE_INFO_PATH, Some(&bound)).unwrap();
	let later = pending.binding.timestamp + 61;
	assert!(store.finish_request_at(pending, GET_NODE_INFO_PATH, b"", later).is_err());
	let timestamp = now();
	let expiry = timestamp + 10;
	let credential = restrict(&admin_token(&store), &[&format!("time-before = {expiry}")]);
	let bound = bind_request(&credential, GET_NODE_INFO_PATH, b"", timestamp);
	let pending = store.authenticate_request(GET_NODE_INFO_PATH, Some(&bound)).unwrap();
	assert_eq!(
		store.finish_request_at(pending, GET_NODE_INFO_PATH, b"", expiry).unwrap_err().error_code,
		LdkServerErrorCode::AuthorizationError
	);
}

// Deliberately bypass reusable-token checks to test rejection of multiple proofs.
fn append_request_proof(token: &str, method: &str, body: &[u8], timestamp: u64) -> String {
	let proof = RequestBinding::new(method, body, timestamp);
	restrict(token, &[&proof.caveat().unwrap()])
}
