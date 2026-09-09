// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Root-file representation and filesystem operations.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hex::DisplayHex;
use ldk_node::bitcoin::hashes::{sha256, Hash};
use serde::Deserialize;

use super::policy::{validate_name_value, validate_permissions};
use super::{invalid_data, MacaroonInfo};
use crate::util::write_new;

pub(super) const MACAROON_FILE_SIZE_LIMIT: usize = 16384;
pub(super) const MACAROONS_DIR: &str = "macaroons";
pub(super) const ADMIN_ROOT_FILE: &str = "admin.toml";
pub(super) const ADMIN_MACAROON_FILE: &str = "admin.macaroon";

#[derive(Debug)]
pub(super) struct RootRecord {
	pub(super) info: Arc<MacaroonInfo>,
	pub(super) secret: String,
	pub(super) path: PathBuf,
}

#[derive(Deserialize)]
pub(super) struct StoredRoot {
	pub(super) id: String,
	pub(super) name: String,
	#[serde(rename = "key")]
	pub(super) secret: String,
	pub(super) permissions: Vec<String>,
	#[serde(default)]
	pub(super) caveats: Vec<String>,
}

pub(super) fn compute_root_id(secret: &str) -> String {
	let hash = sha256::Hash::hash(secret.as_bytes());
	hash[..16].to_lower_hex_string()
}

pub(super) fn record_from_stored(stored: StoredRoot, path: PathBuf) -> io::Result<RootRecord> {
	if !is_hex(&stored.secret, 64) {
		return Err(invalid_data(format!("Invalid macaroon in {}", path.display())));
	}
	if !is_hex(&stored.id, 32) || stored.id != compute_root_id(&stored.secret) {
		return Err(invalid_data(format!("Invalid macaroon ID in {}", path.display())));
	}
	validate_name_value(&stored.name).map_err(invalid_data)?;
	let permissions = validate_permissions(stored.permissions).map_err(invalid_data)?;
	if stored.caveats.len() >= ldk_server_macaroons::MAX_CAVEATS
		|| stored.caveats.iter().any(|c| !c.is_ascii() || c.bytes().any(|b| b < 32 || b == 127))
	{
		return Err(invalid_data("Invalid stored macaroon caveats"));
	}
	Ok(RootRecord {
		info: Arc::new(MacaroonInfo {
			id: stored.id,
			name: stored.name,
			permissions,
			caveats: stored.caveats,
		}),
		secret: stored.secret,
		path,
	})
}

pub(super) fn generate_secret() -> io::Result<String> {
	let mut bytes = [0u8; 32];
	getrandom::getrandom(&mut bytes).map_err(io::Error::other)?;
	Ok(bytes.to_lower_hex_string())
}

pub(super) fn write_root_file(path: &Path, info: &MacaroonInfo, secret: &str) -> io::Result<()> {
	// Rust Debug string escaping is valid TOML for printable ASCII, including quotes
	// and backslashes. Reject controls and Unicode, whose Debug escapes differ from TOML.
	if info.caveats.iter().any(|c| !c.is_ascii() || c.bytes().any(|b| b < 32 || b == 127)) {
		return Err(invalid_data("Invalid stored macaroon caveats"));
	}
	let permissions = info
		.permissions
		.iter()
		.map(|permission| format!("\"{permission}\""))
		.collect::<Vec<_>>()
		.join(", ");
	let contents = format!(
		"id = \"{}\"\nname = \"{}\"\nkey = \"{}\"\npermissions = [{}]\ncaveats = {:?}\n",
		info.id, info.name, secret, permissions, info.caveats
	);

	write_private_file(path, contents.as_bytes())
}

pub(super) fn write_private_file(path: &Path, contents: &[u8]) -> io::Result<()> {
	let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("macaroon");
	let mut suffix = [0u8; 8];
	getrandom::getrandom(&mut suffix).map_err(io::Error::other)?;
	let temporary_path = path.with_file_name(format!(
		".{file_name}.{}.{}.tmp",
		std::process::id(),
		suffix.to_lower_hex_string()
	));
	let result = (|| {
		write_new(&temporary_path, contents, 0o400)?;
		fs::rename(&temporary_path, path)?;
		if let Some(directory) = path.parent() {
			File::open(directory)?.sync_all()?;
		}
		Ok(())
	})();
	if result.is_err() {
		let _ = fs::remove_file(temporary_path);
	}
	result
}

pub(super) fn is_hex(value: &str, expected_length: usize) -> bool {
	value.len() == expected_length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
