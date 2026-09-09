// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Fixtures for server macaroon and HTTP admission tests.

use std::ops::Deref;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

pub(crate) struct TestDir(PathBuf);

impl TestDir {
	pub(crate) fn new(name: &str) -> Self {
		loop {
			let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
			let path = std::env::temp_dir()
				.join(format!("ldk-macaroon-test-{name}-{}-{count}", std::process::id()));
			match fs::create_dir(&path) {
				Ok(()) => return Self(path),
				Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
				Err(error) => panic!("Cannot create test directory: {error}"),
			}
		}
	}
}
impl AsRef<Path> for TestDir {
	fn as_ref(&self) -> &Path {
		&self.0
	}
}
impl Deref for TestDir {
	type Target = Path;
	fn deref(&self) -> &Path {
		&self.0
	}
}
impl Drop for TestDir {
	fn drop(&mut self) {
		let _ = fs::remove_dir_all(&self.0);
	}
}

pub(crate) fn test_store(name: &str) -> (TestDir, MacaroonStore) {
	let directory = TestDir::new(name);
	let store = MacaroonStore::load_or_create(&directory).unwrap();
	(directory, store)
}

pub(crate) fn now() -> u64 {
	unix_time().unwrap()
}

pub(crate) fn restrict(token: &str, caveats: &[&str]) -> String {
	let bytes = Vec::<u8>::from_hex(token).unwrap();
	let mut m = Macaroon::deserialize(&bytes).unwrap();
	for caveat in caveats {
		m.attenuate(caveat.as_bytes()).unwrap();
	}
	m.to_hex()
}

pub(crate) fn admin_token(store: &MacaroonStore) -> String {
	let roots = store.roots.read().unwrap();
	let record = roots.values().find(|r| r.info.name == "admin").unwrap();
	mint_token(&record.info, &record.secret).unwrap()
}

impl MacaroonStore {
	// Unit tests can inspect reusable credentials without creating an HTTP request.
	// Production authorization always requires authenticate_request + finish_request.
	pub(crate) fn authenticate(
		&self, method: &str, auth_header: Option<&str>,
	) -> Result<Arc<MacaroonInfo>, LdkServerError> {
		let token = auth_header.ok_or_else(|| auth_error("Missing macaroon credentials"))?;
		let (macaroon, record) = self.verify_token(token)?;
		Self::authenticate_caveats(&record, macaroon.caveats(), method)
	}
}

pub(crate) fn bind_request(token: &str, method: &str, body: &[u8], timestamp: u64) -> String {
	ldk_server_macaroons::bind_macaroon_to_request_at(token, method, body, timestamp).unwrap()
}
