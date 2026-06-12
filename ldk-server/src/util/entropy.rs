// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::path::Path;
use std::str::FromStr;
use std::{fs, io};

use ldk_node::bip39::Mnemonic;
use ldk_node::entropy::{generate_entropy_mnemonic, NodeEntropy};
use log::info;

use crate::util::write_new;

const DEFAULT_MNEMONIC_FILE: &str = "keys_mnemonic";

pub(crate) fn load_or_generate_node_entropy(storage_dir: &Path) -> io::Result<NodeEntropy> {
	let mnemonic_path = storage_dir.join(DEFAULT_MNEMONIC_FILE);

	let mnemonic = if mnemonic_path.exists() {
		let raw = fs::read_to_string(&mnemonic_path)?;
		Mnemonic::from_str(raw.trim()).map_err(|e| {
			io::Error::new(
				io::ErrorKind::InvalidData,
				format!("Invalid BIP39 mnemonic in {}: {}", mnemonic_path.display(), e),
			)
		})?
	} else {
		if let Some(parent) = mnemonic_path.parent() {
			fs::create_dir_all(parent)?;
		}
		let mnemonic = generate_entropy_mnemonic(None);
		write_new(&mnemonic_path, format!("{}\n", mnemonic).as_bytes(), 0o600)?;
		info!(
			"Generated new BIP39 mnemonic at {}. Back up this file securely — it is required to recover on-chain funds.",
			mnemonic_path.display()
		);
		mnemonic
	};

	Ok(NodeEntropy::from_bip39_mnemonic(mnemonic, None))
}

#[cfg(test)]
mod tests {
	use std::os::unix::fs::{MetadataExt, PermissionsExt};
	use std::path::PathBuf;

	use super::*;

	const STALE_SEED_FILE: &str = "keys_seed";
	const KNOWN_MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

	fn tempdir(tag: &str) -> PathBuf {
		let dir = std::env::temp_dir().join(format!(
			"ldk-server-entropy-test-{}-{}",
			tag,
			std::process::id()
		));
		let _ = fs::remove_dir_all(&dir);
		fs::create_dir_all(&dir).unwrap();
		dir
	}

	#[test]
	fn generates_mnemonic_on_fresh_start() {
		let dir = tempdir("fresh");

		load_or_generate_node_entropy(&dir).unwrap();

		let mnemonic_path = dir.join(DEFAULT_MNEMONIC_FILE);
		assert!(mnemonic_path.exists(), "keys_mnemonic was not created");

		let perms = fs::metadata(&mnemonic_path).unwrap().permissions();
		assert_eq!(perms.mode() & 0o777, 0o600, "expected 0600 permissions");

		let content = fs::read_to_string(&mnemonic_path).unwrap();
		let word_count = content.trim().split_whitespace().count();
		assert_eq!(word_count, 24, "expected 24-word mnemonic, got {}", word_count);

		let mtime_before = fs::metadata(&mnemonic_path).unwrap().mtime();
		load_or_generate_node_entropy(&dir).unwrap();
		let mtime_after = fs::metadata(&mnemonic_path).unwrap().mtime();
		assert_eq!(mtime_before, mtime_after, "mnemonic file was rewritten on second call");
	}

	#[test]
	fn rereads_existing_mnemonic_without_mutation() {
		let dir = tempdir("reread");
		let mnemonic_path = dir.join(DEFAULT_MNEMONIC_FILE);
		fs::write(&mnemonic_path, format!("{}\n", KNOWN_MNEMONIC)).unwrap();
		let bytes_before = fs::read(&mnemonic_path).unwrap();

		load_or_generate_node_entropy(&dir).unwrap();

		let bytes_after = fs::read(&mnemonic_path).unwrap();
		assert_eq!(bytes_before, bytes_after, "mnemonic file content changed");
	}

	#[test]
	fn default_entropy_ignores_stale_keys_seed() {
		let dir = tempdir("stale-seed");
		let stale_seed_path = dir.join(STALE_SEED_FILE);
		fs::write(&stale_seed_path, vec![0x42u8; 64]).unwrap();

		load_or_generate_node_entropy(&dir).unwrap();

		assert!(dir.join(DEFAULT_MNEMONIC_FILE).exists(), "keys_mnemonic was not created");
		assert!(stale_seed_path.exists(), "stale keys_seed was removed");
	}

	#[test]
	fn rejects_invalid_mnemonic_file() {
		let dir = tempdir("invalid");
		fs::write(
			dir.join(DEFAULT_MNEMONIC_FILE),
			"these words are definitely not a valid bip39 phrase at all nope",
		)
		.unwrap();

		let err = load_or_generate_node_entropy(&dir).unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::InvalidData);
	}
}
