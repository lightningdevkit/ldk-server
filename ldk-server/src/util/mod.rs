// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

pub(crate) mod config;
pub(crate) mod logger;
pub(crate) mod metrics;
pub(crate) mod proto_adapter;
pub(crate) mod systemd;
pub(crate) mod tls;

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

pub(crate) fn write_new(path: &Path, contents: &[u8], mode: u32) -> io::Result<()> {
	let mut file = OpenOptions::new().create_new(true).write(true).mode(mode).open(path)?;
	file.write_all(contents)?;
	fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
	file.sync_all()?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	#[test]
	fn write_new_sets_requested_mode_and_contents() {
		let dir = test_dir("mode_and_contents");
		let path = dir.join("secret");

		write_new(&path, b"secret-bytes", 0o400).unwrap();

		assert_eq!(fs::read(&path).unwrap(), b"secret-bytes");
		assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o400);

		fs::remove_dir_all(dir).unwrap();
	}

	#[test]
	fn write_new_does_not_replace_existing_file() {
		let dir = test_dir("existing_file");
		let path = dir.join("secret");
		fs::write(&path, b"original").unwrap();

		let err = write_new(&path, b"replacement", 0o400).unwrap_err();

		assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
		assert_eq!(fs::read(&path).unwrap(), b"original");

		fs::remove_dir_all(dir).unwrap();
	}

	fn test_dir(name: &str) -> PathBuf {
		let dir = std::env::temp_dir()
			.join(format!("ldk-server-secure-file-test-{name}-{}", std::process::id()));
		let _ = fs::remove_dir_all(&dir);
		fs::create_dir(&dir).unwrap();
		dir
	}
}
