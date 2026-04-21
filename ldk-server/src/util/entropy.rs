// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use ldk_node::bip39::Mnemonic;
use ldk_node::entropy::{generate_entropy_mnemonic, NodeEntropy};
use log::info;

use crate::util::config::EntropyConfig;

const DEFAULT_MNEMONIC_FILE: &str = "keys_mnemonic";
const LEGACY_SEED_FILE: &str = "keys_seed";

pub(crate) fn load_or_generate_node_entropy(
	storage_dir: &Path, entropy_config: &EntropyConfig,
) -> io::Result<NodeEntropy> {
	if let Some(seed_file) = &entropy_config.seed_file {
		info!("Loading node entropy from raw seed file at {}", seed_file);
		return NodeEntropy::from_seed_path(seed_file.clone())
			.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()));
	}

	let legacy_seed_path = storage_dir.join(LEGACY_SEED_FILE);
	if entropy_config.mnemonic_file.is_none() && legacy_seed_path.exists() {
		info!(
			"Detected legacy raw seed file at {}; continuing to use it. New installs use a BIP39 mnemonic by default.",
			legacy_seed_path.display()
		);
		return NodeEntropy::from_seed_path(legacy_seed_path.to_string_lossy().into_owned())
			.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()));
	}

	let mnemonic_path = match &entropy_config.mnemonic_file {
		Some(p) => PathBuf::from(p),
		None => storage_dir.join(DEFAULT_MNEMONIC_FILE),
	};

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
		write_mnemonic_file(&mnemonic_path, &mnemonic)?;
		info!(
			"Generated new BIP39 mnemonic at {}. Back up this file securely — it is required to recover on-chain funds.",
			mnemonic_path.display()
		);
		mnemonic
	};

	Ok(NodeEntropy::from_bip39_mnemonic(mnemonic, None))
}

fn write_mnemonic_file(path: &Path, mnemonic: &Mnemonic) -> io::Result<()> {
	let mut f = fs::OpenOptions::new().create_new(true).write(true).open(path)?;
	writeln!(f, "{}", mnemonic)?;
	f.sync_all()?;
	fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::os::unix::fs::MetadataExt;

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
		let cfg = EntropyConfig::default();

		load_or_generate_node_entropy(&dir, &cfg).unwrap();

		let mnemonic_path = dir.join(DEFAULT_MNEMONIC_FILE);
		assert!(mnemonic_path.exists(), "keys_mnemonic was not created");

		let perms = fs::metadata(&mnemonic_path).unwrap().permissions();
		assert_eq!(perms.mode() & 0o777, 0o600, "expected 0600 permissions");

		let content = fs::read_to_string(&mnemonic_path).unwrap();
		let word_count = content.trim().split_whitespace().count();
		assert_eq!(word_count, 24, "expected 24-word mnemonic, got {}", word_count);

		let mtime_before = fs::metadata(&mnemonic_path).unwrap().mtime();
		load_or_generate_node_entropy(&dir, &cfg).unwrap();
		let mtime_after = fs::metadata(&mnemonic_path).unwrap().mtime();
		assert_eq!(mtime_before, mtime_after, "mnemonic file was rewritten on second call");
	}

	#[test]
	fn rereads_existing_mnemonic_without_mutation() {
		let dir = tempdir("reread");
		let mnemonic_path = dir.join(DEFAULT_MNEMONIC_FILE);
		fs::write(&mnemonic_path, format!("{}\n", KNOWN_MNEMONIC)).unwrap();
		let bytes_before = fs::read(&mnemonic_path).unwrap();

		load_or_generate_node_entropy(&dir, &EntropyConfig::default()).unwrap();

		let bytes_after = fs::read(&mnemonic_path).unwrap();
		assert_eq!(bytes_before, bytes_after, "mnemonic file content changed");
	}

	#[test]
	fn auto_detects_legacy_keys_seed() {
		let dir = tempdir("legacy");
		let legacy_path = dir.join(LEGACY_SEED_FILE);
		fs::write(&legacy_path, vec![0x42u8; 64]).unwrap();

		load_or_generate_node_entropy(&dir, &EntropyConfig::default()).unwrap();

		assert!(
			!dir.join(DEFAULT_MNEMONIC_FILE).exists(),
			"keys_mnemonic was unexpectedly created"
		);
		assert!(legacy_path.exists(), "legacy keys_seed was removed");
	}

	#[test]
	fn explicit_seed_file_used_directly() {
		let dir = tempdir("explicit-seed");
		let custom_seed = dir.join("custom-seed.bin");
		fs::write(&custom_seed, vec![0x17u8; 64]).unwrap();

		let cfg = EntropyConfig {
			seed_file: Some(custom_seed.to_string_lossy().into_owned()),
			mnemonic_file: None,
		};

		load_or_generate_node_entropy(&dir, &cfg).unwrap();

		assert!(
			!dir.join(DEFAULT_MNEMONIC_FILE).exists(),
			"keys_mnemonic was created despite seed_file being set"
		);
	}

	#[test]
	fn rejects_invalid_mnemonic_file() {
		let dir = tempdir("invalid");
		fs::write(
			dir.join(DEFAULT_MNEMONIC_FILE),
			"these words are definitely not a valid bip39 phrase at all nope",
		)
		.unwrap();

		let err = load_or_generate_node_entropy(&dir, &EntropyConfig::default()).unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::InvalidData);
	}

	#[test]
	fn custom_mnemonic_path_respected() {
		let dir = tempdir("custom-mnemonic");
		let custom_path = dir.join("elsewhere").join("my_mnemonic");
		let cfg = EntropyConfig {
			mnemonic_file: Some(custom_path.to_string_lossy().into_owned()),
			seed_file: None,
		};

		load_or_generate_node_entropy(&dir, &cfg).unwrap();

		assert!(custom_path.exists(), "custom mnemonic file was not created");
		assert!(
			!dir.join(DEFAULT_MNEMONIC_FILE).exists(),
			"default keys_mnemonic was unexpectedly created"
		);

		let content_before = fs::read(&custom_path).unwrap();
		load_or_generate_node_entropy(&dir, &cfg).unwrap();
		let content_after = fs::read(&custom_path).unwrap();
		assert_eq!(content_before, content_after);
	}
}
