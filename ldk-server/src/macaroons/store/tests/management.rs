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
fn creates_lists_revokes_and_reloads_root() {
	let (directory, store) = test_store("lifecycle");
	let admin = store.list_roots().unwrap().remove(0);
	let created =
		store.create_root("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();

	assert_eq!(store.list_roots().unwrap().len(), 2);
	assert!(created.info.allows(NODE_READ_PERMISSION));
	assert!(!created.info.is_admin());
	drop(store);

	let reloaded = MacaroonStore::load_or_create(&directory).unwrap();
	assert_eq!(reloaded.list_roots().unwrap().len(), 2);
	reloaded.revoke_root(&created.info.id, &admin).unwrap();
	assert_eq!(reloaded.list_roots().unwrap(), vec![admin]);
}

#[test]
fn revokes_root_when_its_file_is_missing() {
	let (directory, store) = test_store("revoke-missing-file");
	let admin = store.list_roots().unwrap().remove(0);
	let reader =
		store.create_root("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();
	let header = reader.token.clone();
	fs::remove_file(
		directory.join(MACAROONS_DIR).join("roots").join(format!("{}.toml", reader.info.id)),
	)
	.unwrap();
	assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&header)).is_ok());

	store.revoke_root(&reader.info.id, &admin).unwrap();
	assert_eq!(
		store.authenticate(GET_NODE_INFO_PATH, Some(&header)).unwrap_err().error_code,
		LdkServerErrorCode::AuthError
	);
	assert_eq!(store.list_roots().unwrap(), vec![admin.clone()]);
	assert_eq!(
		MacaroonStore::load_or_create(&directory).unwrap().list_roots().unwrap(),
		vec![admin]
	);
}

#[test]
fn revoke_rejects_malformed_ids_without_echoing_them() {
	let (_directory, store) = test_store("revoke-invalid-id");
	let admin = store.list_roots().unwrap().remove(0);
	for id in [String::new(), "a".repeat(31), "a".repeat(33), "z".repeat(32), "a".repeat(8192)] {
		let error = store.revoke_root(&id, &admin).unwrap_err();
		assert_eq!(error.error_code, LdkServerErrorCode::InvalidRequestError);
		assert_eq!(error.message, "macaroon ID must contain exactly 32 hexadecimal characters");
	}
	assert_eq!(store.list_roots().unwrap(), vec![admin]);
}

#[tokio::test]
async fn authentication_continues_during_root_file_write() {
	use std::time::Duration;
	let directory = TestDir::new("slow-root-write");
	let store = Arc::new(MacaroonStore::load_or_create(&directory).unwrap());
	let admin = store.list_roots().unwrap().remove(0);
	let reader =
		store.create_root("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();
	let header = reader.token.clone();
	let first = store.authenticate(GET_NODE_INFO_PATH, Some(&header)).unwrap();
	let (started_tx, started_rx) = tokio::sync::oneshot::channel();
	let (release_tx, release_rx) = std::sync::mpsc::channel();
	let writer_store = Arc::clone(&store);
	let writer = tokio::task::spawn_blocking(move || {
		writer_store.create_root_with_writer(
			"pending",
			vec![NODE_READ_PERMISSION.to_string()],
			&admin,
			|path, info, secret| {
				started_tx.send(()).unwrap();
				release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
				write_root_file(path, info, secret)
			},
		)
	});
	started_rx.await.unwrap();
	let auth_store = Arc::clone(&store);
	let auth = tokio::task::spawn_blocking(move || {
		let info = auth_store.authenticate(GET_NODE_INFO_PATH, Some(&header)).unwrap();
		assert!(!auth_store.list_roots().unwrap().iter().any(|info| info.name == "pending"));
		info
	});
	let result = tokio::time::timeout(Duration::from_secs(1), auth).await;
	// Release the writer even if authentication timed out, so the test cannot hang.
	release_tx.send(()).unwrap();
	writer.await.unwrap().unwrap();
	let second = result.expect("Authentication waited for disk I/O").unwrap();
	assert_eq!(first, second);
	assert!(store.list_roots().unwrap().iter().any(|info| info.name == "pending"));
}

#[test]
fn failed_root_write_does_not_publish_root() {
	let (_directory, store) = test_store("failed-root-write");
	let admin = store.list_roots().unwrap().remove(0);
	let error = store
		.create_root_with_writer(
			"reader",
			vec![NODE_READ_PERMISSION.to_string()],
			&admin,
			|_, _, _| Err(io::Error::other("injected write failure")),
		)
		.unwrap_err();
	assert_eq!(error.error_code, LdkServerErrorCode::InternalServerError);
	assert_eq!(store.list_roots().unwrap(), vec![admin.clone()]);
	store.create_root("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();
}

#[test]
fn revoked_issuer_cannot_manage_roots_with_an_old_snapshot() {
	let (_directory, store) = test_store("revoked-issuer");
	let admin = store.list_roots().unwrap().remove(0);
	let manager = store
		.create_root(
			"manager",
			vec![MACAROONS_MANAGE_PERMISSION.to_string(), NODE_READ_PERMISSION.to_string()],
			&admin,
		)
		.unwrap()
		.info;
	let reader =
		store.create_root("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();
	store.revoke_root(&manager.id, &admin).unwrap();
	assert_eq!(
		store
			.create_root("late", vec![NODE_READ_PERMISSION.to_string()], &manager)
			.unwrap_err()
			.error_code,
		LdkServerErrorCode::AuthError
	);
	assert_eq!(
		store.revoke_root(&reader.info.id, &manager).unwrap_err().error_code,
		LdkServerErrorCode::AuthError
	);
	assert!(store.list_roots().unwrap().contains(&reader.info));
}

#[test]
fn concurrent_creates_keep_root_names_unique() {
	let (directory, store) = test_store("concurrent-create");
	let admin = store.list_roots().unwrap().remove(0);
	let barrier = std::sync::Barrier::new(4);
	std::thread::scope(|scope| {
		let handles: Vec<_> = (0..4)
			.map(|_| {
				scope.spawn(|| {
					barrier.wait();
					store.create_root("reader", vec![NODE_READ_PERMISSION.to_string()], &admin)
				})
			})
			.collect();
		let results: Vec<_> = handles.into_iter().map(|handle| handle.join().unwrap()).collect();
		assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
		for error in results.into_iter().filter_map(Result::err) {
			assert_eq!(error.error_code, LdkServerErrorCode::InvalidRequestError);
		}
	});
	let reloaded = MacaroonStore::load_or_create(&directory).unwrap();
	assert_eq!(store.list_roots().unwrap(), reloaded.list_roots().unwrap());
	assert_eq!(reloaded.list_roots().unwrap().len(), 2);
}

#[test]
fn scoped_manager_cannot_escalate_or_revoke_admin() {
	let (_directory, store) = test_store("delegation");
	let admin = store.list_roots().unwrap().remove(0);
	let manager = store
		.create_root(
			"manager",
			vec![MACAROONS_MANAGE_PERMISSION.to_string(), NODE_READ_PERMISSION.to_string()],
			&admin,
		)
		.unwrap()
		.info;

	let delegated =
		store.create_root("delegated", vec![NODE_READ_PERMISSION.to_string()], &manager).unwrap();
	assert!(delegated.info.allows(NODE_READ_PERMISSION));
	assert_eq!(
		store
			.create_root("escalated", vec![ADMIN_PERMISSION.to_string()], &manager)
			.unwrap_err()
			.error_code,
		LdkServerErrorCode::AuthorizationError
	);
	assert_eq!(
		store.revoke_root(&admin.id, &manager).unwrap_err().error_code,
		LdkServerErrorCode::AuthorizationError
	);
}

#[test]
fn scoped_manager_revokes_only_roots_within_its_permissions() {
	let (directory, store) = test_store("scoped-revocation");
	let admin = store.list_roots().unwrap().remove(0);
	let manager = store
		.create_root(
			"manager",
			vec![MACAROONS_MANAGE_PERMISSION.to_string(), NODE_READ_PERMISSION.to_string()],
			&admin,
		)
		.unwrap()
		.info;
	let reader =
		store.create_root("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap().info;
	let peer = store
		.create_root(
			"peer",
			vec![NODE_READ_PERMISSION.to_string(), PAYMENTS_SEND_PERMISSION.to_string()],
			&admin,
		)
		.unwrap()
		.info;

	assert_eq!(
		store.revoke_root(&peer.id, &manager).unwrap_err().error_code,
		LdkServerErrorCode::AuthorizationError
	);
	store.revoke_root(&reader.id, &manager).unwrap();
	let roots = store.list_roots().unwrap();
	assert!(roots.contains(&peer));
	assert!(!roots.contains(&reader));
	assert_eq!(MacaroonStore::load_or_create(&directory).unwrap().list_roots().unwrap(), roots);
}

#[test]
fn concurrent_revocations_preserve_the_final_admin() {
	let (directory, store) = test_store("concurrent-revoke");
	let first = store.list_roots().unwrap().remove(0);
	let second =
		store.create_root("second-admin", vec![ADMIN_PERMISSION.to_string()], &first).unwrap().info;
	let barrier = std::sync::Barrier::new(2);
	std::thread::scope(|scope| {
		let handles: Vec<_> = [&first, &second]
			.into_iter()
			.map(|admin| {
				let store = &store;
				let barrier = &barrier;
				scope.spawn(move || {
					barrier.wait();
					store.revoke_root(&admin.id, admin)
				})
			})
			.collect();
		let results: Vec<_> = handles.into_iter().map(|handle| handle.join().unwrap()).collect();
		assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
		let error = results.into_iter().find_map(Result::err).unwrap();
		assert_eq!(error.error_code, LdkServerErrorCode::InvalidRequestError);
	});
	let roots = store.list_roots().unwrap();
	assert_eq!(roots.len(), 1);
	assert!(roots[0].is_admin());
	assert_eq!(MacaroonStore::load_or_create(&directory).unwrap().list_roots().unwrap(), roots);
}

#[test]
fn refuses_to_revoke_final_admin() {
	let (_directory, store) = test_store("final-admin");
	let admin = store.list_roots().unwrap().remove(0);

	let error = store.revoke_root(&admin.id, &admin).unwrap_err();
	assert_eq!(error.error_code, LdkServerErrorCode::InvalidRequestError);
	assert!(store.roots.read().unwrap().contains_key(&admin.id));
}

#[test]
fn issued_children_inherit_expiry_and_method_restrictions() {
	let (directory, store) = test_store("inherited-caveats");
	let expiry = format!("time-before = {}", now() + 3600);
	let token = restrict(&admin_token(&store), &[&expiry, "method = CreateMacaroon"]);
	let issuer = store.authenticate(CREATE_MACAROON_PATH, Some(&token)).unwrap();
	let created = store.create_root("child", vec![ADMIN_PERMISSION.into()], &issuer).unwrap();
	assert!(created.info.caveats.contains(&expiry));
	assert!(created.info.caveats.contains(&"method = CreateMacaroon".to_string()));
	assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&created.token)).is_err());
	let child_issuer = store.authenticate(CREATE_MACAROON_PATH, Some(&created.token)).unwrap();
	let grandchild =
		store.create_root("grandchild", vec![NODE_READ_PERMISSION.into()], &child_issuer).unwrap();
	assert!(grandchild.info.caveats.contains(&expiry));
	assert!(grandchild.info.caveats.contains(&"method = CreateMacaroon".to_string()));
	assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&grandchild.token)).is_err());
	let reloaded = MacaroonStore::load_or_create(&directory).unwrap();
	assert!(reloaded.authenticate(GET_NODE_INFO_PATH, Some(&created.token)).is_err());
	let mut stale = (*issuer).clone();
	stale.caveats.push("time-before = 0".into());
	assert!(store.create_root("expired", vec![NODE_READ_PERMISSION.into()], &stale).is_err());
}

#[test]
fn restricted_admin_does_not_replace_the_last_unrestricted_admin() {
	let (_directory, store) = test_store("restricted-admin");
	let token = restrict(&admin_token(&store), &["method = CreateMacaroon"]);
	let issuer = store.authenticate(CREATE_MACAROON_PATH, Some(&token)).unwrap();
	store.create_root("restricted-admin", vec![ADMIN_PERMISSION.into()], &issuer).unwrap();
	let admin = store.list_roots().unwrap().into_iter().find(|i| i.name == "admin").unwrap();
	assert!(store.revoke_root(&admin.id, &admin).is_err());
}

#[test]
fn stored_caveats_are_reported_and_inherited_through_grandchildren() {
	let (directory, store) = test_store("stored-inheritance");
	let token = admin_token(&store);
	let expiry = format!("time-before = {}", now() + 3600);
	{
		let roots = store.roots.read().unwrap();
		let record = roots.values().next().unwrap();
		let mut info = (*record.info).clone();
		info.caveats = vec![expiry.clone(), expiry.clone()];
		write_root_file(&record.path, &info, &record.secret).unwrap();
	}
	drop(store);
	let store = MacaroonStore::load_or_create(&directory).unwrap();
	let issuer = store.authenticate(GET_PERMISSIONS_PATH, Some(&token)).unwrap();
	assert_eq!(issuer.caveats, vec![expiry.clone(), "permissions = admin".into()]);
	let child = store.create_root("child", vec![ADMIN_PERMISSION.into()], &issuer).unwrap();
	let child_issuer = store.authenticate(CREATE_MACAROON_PATH, Some(&child.token)).unwrap();
	assert_eq!(child_issuer.caveats, issuer.caveats);
	let grandchild =
		store.create_root("grandchild", vec![NODE_READ_PERMISSION.into()], &child_issuer).unwrap();
	let grandchild_info =
		store.authenticate(GET_PERMISSIONS_PATH, Some(&grandchild.token)).unwrap();
	assert_eq!(grandchild_info.caveats.iter().filter(|c| **c == expiry).count(), 1);
	assert_eq!(grandchild_info.permissions, BTreeSet::from([NODE_READ_PERMISSION.into()]));
	assert!(grandchild.info.caveats.contains(&expiry));
	let reloaded = MacaroonStore::load_or_create(&directory).unwrap();
	assert_eq!(
		reloaded.authenticate(GET_PERMISSIONS_PATH, Some(&grandchild.token)).unwrap(),
		grandchild_info
	);
}

#[test]
fn uppercase_credentials_revoke_and_names_can_be_reused() {
	let (directory, store) = test_store("uppercase");
	let token = admin_token(&store).to_ascii_uppercase();
	let admin = store.authenticate(CREATE_MACAROON_PATH, Some(&token)).unwrap();
	let child = store.create_root("reusable", vec![NODE_READ_PERMISSION.into()], &admin).unwrap();
	assert!(store
		.authenticate(GET_NODE_INFO_PATH, Some(&child.token.to_ascii_uppercase()))
		.is_ok());
	store.revoke_root(&child.info.id.to_ascii_uppercase(), &admin).unwrap();
	assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&child.token)).is_err());
	let replacement =
		store.create_root("reusable", vec![NODE_READ_PERMISSION.into()], &admin).unwrap();
	assert_ne!(replacement.info.id, child.info.id);
	let reloaded = MacaroonStore::load_or_create(&directory).unwrap();
	assert!(reloaded.authenticate(GET_NODE_INFO_PATH, Some(&child.token)).is_err());
	assert!(reloaded.authenticate(GET_NODE_INFO_PATH, Some(&replacement.token)).is_ok());
}

#[test]
fn issued_tokens_reserve_capacity_for_request_proofs() {
	let (_directory, store) = test_store("request-proof-capacity");
	let mut admin = store.list_roots().unwrap().remove(0);
	admin.caveats = vec!["permissions = admin".into(); ldk_server_macaroons::MAX_CAVEATS - 2];
	let child = store.create_root("fits", vec![ADMIN_PERMISSION.into()], &admin).unwrap();
	let bound = bind_request(&child.token, GET_NODE_INFO_PATH, b"", now());
	assert!(store.authenticate_request(GET_NODE_INFO_PATH, Some(&bound)).is_ok());
	admin.caveats.push("permissions = admin".into());
	assert!(store.create_root("too-many", vec![ADMIN_PERMISSION.into()], &admin).is_err());
	assert!(!store.list_roots().unwrap().iter().any(|info| info.name == "too-many"));
}
