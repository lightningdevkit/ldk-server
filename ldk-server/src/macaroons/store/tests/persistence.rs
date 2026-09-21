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
fn load_macaroon_rejects_oversized_toml() {
	let (directory, store) = test_store("oversized-root-toml");
	let path = directory.join(MACAROONS_DIR).join("roots").join("oversized.toml");
	fs::write(&path, vec![b' '; MACAROON_FILE_SIZE_LIMIT + 1]).unwrap();
	drop(store);
	let error = MacaroonStore::load_or_create(&directory).err().unwrap();
	assert_eq!(error.kind(), io::ErrorKind::InvalidData);
	assert!(error.to_string().contains("exceeds"));
}

#[test]
fn load_rejects_duplicate_root_names_and_ids() {
	for duplicate in ["name", "ID"] {
		let (directory, store) = test_store("duplicate-root");
		let (mut info, mut secret) = {
			let roots = store.roots.read().unwrap();
			let admin = roots.values().next().unwrap();
			((*admin.info).clone(), admin.secret.clone())
		};
		if duplicate == "name" {
			// Same name, but a different valid secret and ID.
			secret = generate_secret().unwrap();
			info.id = compute_root_id(&secret);
		} else {
			// Same secret and ID, but a different valid name.
			info.name = "another-admin".to_string();
		}
		write_root_file(
			&directory.join(MACAROONS_DIR).join("roots").join("duplicate.toml"),
			&info,
			&secret,
		)
		.unwrap();
		drop(store);

		let error = MacaroonStore::load_or_create(&directory).err().unwrap();
		assert_eq!(error.kind(), io::ErrorKind::InvalidData);
		assert!(error.to_string().contains(&format!("Duplicate macaroon {duplicate}")));
	}
}

#[test]
fn bootstrap_recovers_after_roots_reset_and_invalid_token() {
	let (directory, store) = test_store("bootstrap-reset");
	let path = directory.join(MACAROONS_DIR).join(ADMIN_MACAROON_FILE);
	let original = fs::read_to_string(&path).unwrap();
	drop(store);
	fs::remove_dir_all(directory.join(MACAROONS_DIR).join("roots")).unwrap();
	let store = MacaroonStore::load_or_create(&directory).unwrap();
	let replacement = fs::read_to_string(&path).unwrap();
	assert_ne!(replacement, original);
	assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&original)).is_err());
	assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&replacement)).unwrap().is_admin());
	drop(store);
	let mut forged = Vec::<u8>::from_hex(&replacement).unwrap();
	*forged.last_mut().unwrap() ^= 1;
	for invalid in [
		original,
		forged.to_lower_hex_string(),
		"malformed".into(),
		"a".repeat(MAX_MACAROON_BYTES * 2 + 3),
	] {
		write_private_file(&path, invalid.as_bytes()).unwrap();
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		assert_eq!(fs::read_to_string(&path).unwrap(), replacement);
		assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&replacement)).is_ok());
	}
}

#[test]
fn bootstrap_read_errors_identify_the_default_token_path() {
	let directory = TestDir::new("bootstrap-read-error");
	MacaroonStore::load_or_create(&directory).unwrap();
	let path = directory.join(MACAROONS_DIR).join(ADMIN_MACAROON_FILE);
	fs::remove_file(&path).unwrap();
	fs::create_dir(&path).unwrap();
	let expected_kind = fs::read_to_string(&path).unwrap_err().kind();
	let error = MacaroonStore::load_or_create(&directory).err().unwrap();
	assert_eq!(error.kind(), expected_kind);
	assert!(error.to_string().contains(path.to_str().unwrap()));
}

#[test]
fn stored_caveat_serialization_roundtrips_printable_ascii() {
	let (_directory, store) = test_store("caveat-escaping");
	let roots = store.roots.read().unwrap();
	let record = roots.values().next().unwrap();
	let mut info = (*record.info).clone();
	info.caveats = vec![(32u8..127).map(char::from).collect()];
	write_root_file(&record.path, &info, &record.secret).unwrap();
	let stored: StoredRoot = toml::from_str(&fs::read_to_string(&record.path).unwrap()).unwrap();
	assert_eq!(stored.caveats, info.caveats);
	for invalid in ["control\0", "newline\n", "unicode é"] {
		info.caveats = vec![invalid.into()];
		assert!(write_root_file(&record.path, &info, &record.secret).is_err());
	}
}
