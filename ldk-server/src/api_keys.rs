// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::collections::{HashMap, HashSet};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use hex::DisplayHex;
use ldk_node::bitcoin::hashes::{sha256, Hash};

use crate::api::error::LdkServerError;
use crate::api::error::LdkServerErrorCode::{AuthError, InvalidRequestError};

/// Computes the key_id for an API key: first 8 bytes of SHA256(key_hex), hex-encoded (16 chars).
pub fn compute_key_id(key_hex: &str) -> String {
	let hash = sha256::Hash::hash(key_hex.as_bytes());
	hash[..8].to_lower_hex_string()
}

/// Atomically writes contents to path by writing to a temp file then renaming.
/// Sets permissions to 0o400 (read-only for owner).
fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
	let tmp_path = path.with_file_name(format!(".tmp_{}", std::process::id(),));
	std::fs::write(&tmp_path, contents)?;
	std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o400))?;
	std::fs::rename(&tmp_path, path)?;
	Ok(())
}

/// Stores and manages API keys with per-endpoint permissions.
/// Keys are stored as TOML files in the `api_keys/` directory.
/// The HashMap maps key_id -> (key_hex, endpoints).
/// An empty endpoints set means the key is an admin key with access to all endpoints.
pub struct ApiKeyStore {
	keys: HashMap<String, (String, HashSet<String>)>,
	api_keys_dir: PathBuf,
}

impl ApiKeyStore {
	/// Loads all API key files from the given directory.
	/// Each file should be a `.toml` file containing `key` and optionally `endpoints` fields.
	pub fn load_from_dir(api_keys_dir: &Path) -> io::Result<Self> {
		let mut keys = HashMap::new();

		if api_keys_dir.exists() {
			for entry in std::fs::read_dir(api_keys_dir)? {
				let entry = entry?;
				let path = entry.path();
				if path.extension().is_none_or(|ext| ext != "toml") {
					continue;
				}

				let contents = std::fs::read_to_string(&path)?;
				let parsed: toml::Value = toml::from_str(&contents).map_err(|e| {
					io::Error::new(
						io::ErrorKind::InvalidData,
						format!("Failed to parse {}: {}", path.display(), e),
					)
				})?;

				let key_hex = parsed
					.get("key")
					.and_then(|v| v.as_str())
					.ok_or_else(|| {
						io::Error::new(
							io::ErrorKind::InvalidData,
							format!("Missing 'key' field in {}", path.display()),
						)
					})?
					.to_string();

				// Validate 64-char hex key
				if key_hex.len() != 64 || !key_hex.chars().all(|c| c.is_ascii_hexdigit()) {
					return Err(io::Error::new(
						io::ErrorKind::InvalidData,
						format!("Invalid key format in {}: must be 64 hex chars", path.display()),
					));
				}

				let endpoints: HashSet<String> = parsed
					.get("endpoints")
					.and_then(|v| v.as_array())
					.map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
					.unwrap_or_default();

				let key_id = compute_key_id(&key_hex);
				keys.insert(key_id, (key_hex, endpoints));
			}
		}

		Ok(Self { keys, api_keys_dir: api_keys_dir.to_path_buf() })
	}

	/// Initializes the api_keys directory, migrating any legacy api_key file.
	/// Returns the path to the api_keys directory.
	pub fn init(storage_dir: &Path) -> io::Result<PathBuf> {
		let api_keys_dir = storage_dir.join("api_keys");
		std::fs::create_dir_all(&api_keys_dir)?;

		let admin_toml = api_keys_dir.join("admin.toml");

		// TODO: Remove legacy migration once all deployments have been upgraded.
		let legacy_path = storage_dir.join("api_key");
		if legacy_path.exists() && !admin_toml.exists() {
			let key_bytes = std::fs::read(&legacy_path)?;
			let key_hex = key_bytes.to_lower_hex_string();
			let toml_contents = format!("key = \"{key_hex}\"\nendpoints = [\"*\"]\n");
			atomic_write(&admin_toml, toml_contents.as_bytes())?;
			return Ok(api_keys_dir);
		}

		if !admin_toml.exists() {
			let mut key_bytes = [0u8; 32];
			getrandom::getrandom(&mut key_bytes).map_err(io::Error::other)?;
			let key_hex = key_bytes.to_lower_hex_string();
			let toml_contents = format!("key = \"{key_hex}\"\nendpoints = [\"*\"]\n");
			atomic_write(&admin_toml, toml_contents.as_bytes())?;
		}

		Ok(api_keys_dir)
	}

	/// Validates authentication and checks endpoint authorization.
	/// Returns the set of permitted endpoints for this key.
	pub fn validate_and_authorize(
		&self, endpoint: &str, key_id: &str, timestamp: u64, hmac_hex: &str, body: &[u8],
	) -> Result<HashSet<String>, LdkServerError> {
		use ldk_node::bitcoin::hashes::hmac::{Hmac, HmacEngine};
		use ldk_node::bitcoin::hashes::HashEngine;

		// O(1) lookup by key_id
		let (key_hex, endpoints) = self
			.keys
			.get(key_id)
			.ok_or_else(|| LdkServerError::new(AuthError, "Invalid credentials"))?;

		// Validate timestamp is within acceptable window
		let now = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map_err(|_| LdkServerError::new(AuthError, "System time error"))?
			.as_secs();

		let time_diff = now.abs_diff(timestamp);
		if time_diff > super::service::AUTH_TIMESTAMP_TOLERANCE_SECS {
			return Err(LdkServerError::new(AuthError, "Request timestamp expired"));
		}

		// Compute expected HMAC: HMAC-SHA256(api_key, timestamp_bytes || body)
		let mut hmac_engine: HmacEngine<sha256::Hash> = HmacEngine::new(key_hex.as_bytes());
		hmac_engine.input(&timestamp.to_be_bytes());
		hmac_engine.input(body);
		let expected_hmac = Hmac::<sha256::Hash>::from_engine(hmac_engine);

		// Compare HMACs (constant-time comparison via Hash equality)
		let expected_hex = expected_hmac.to_string();
		if expected_hex != hmac_hex {
			return Err(LdkServerError::new(AuthError, "Invalid credentials"));
		}

		// GetPermissions is always allowed
		if endpoint == ldk_server_protos::endpoints::GET_PERMISSIONS_PATH {
			return Ok(endpoints.clone());
		}

		// Check endpoint permission — "*" means admin (all endpoints allowed)
		if !endpoints.contains("*") && !endpoints.contains(endpoint) {
			return Err(LdkServerError::new(
				AuthError,
				format!("Key not authorized for endpoint: {}", endpoint),
			));
		}

		Ok(endpoints.clone())
	}

	/// Creates a new API key with the given name and endpoint permissions.
	/// Returns the hex-encoded key.
	pub fn create_key(
		&mut self, name: &str, endpoints: Vec<String>,
	) -> Result<String, LdkServerError> {
		// Validate name: alphanumeric, hyphens, underscores only
		if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
			return Err(LdkServerError::new(
				InvalidRequestError,
				"Name must be non-empty and contain only alphanumeric characters, hyphens, or underscores",
			));
		}

		if endpoints.is_empty() {
			return Err(LdkServerError::new(
				InvalidRequestError,
				"Endpoints list must not be empty",
			));
		}

		// Validate endpoint names
		for ep in &endpoints {
			if ep != "*" && !ldk_server_protos::endpoints::ALL_ENDPOINTS.contains(&ep.as_str()) {
				return Err(LdkServerError::new(
					InvalidRequestError,
					format!("Unknown endpoint: '{}'", ep),
				));
			}
		}

		// Check for duplicate file name
		let toml_path = self.api_keys_dir.join(format!("{}.toml", name));
		if toml_path.exists() {
			return Err(LdkServerError::new(
				InvalidRequestError,
				format!("API key with name '{}' already exists", name),
			));
		}

		// Generate 32-byte random key
		let mut key_bytes = [0u8; 32];
		getrandom::getrandom(&mut key_bytes).map_err(|e| {
			LdkServerError::new(InvalidRequestError, format!("Failed to generate key: {}", e))
		})?;
		let key_hex = key_bytes.to_lower_hex_string();

		// Build TOML content
		let endpoints_toml: Vec<String> = endpoints.iter().map(|e| format!("\"{}\"", e)).collect();
		let toml_contents =
			format!("key = \"{}\"\nendpoints = [{}]\n", key_hex, endpoints_toml.join(", "));

		// Write to disk
		atomic_write(&toml_path, toml_contents.as_bytes()).map_err(|e| {
			LdkServerError::new(InvalidRequestError, format!("Failed to write API key file: {}", e))
		})?;

		// Update in-memory store
		let key_id = compute_key_id(&key_hex);
		let endpoint_set: HashSet<String> = endpoints.into_iter().collect();
		self.keys.insert(key_id, (key_hex.clone(), endpoint_set));

		Ok(key_hex)
	}
}

#[cfg(test)]
mod tests {
	use std::fs;
	use std::sync::atomic::{AtomicU32, Ordering};

	use super::*;

	static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

	fn test_dir(name: &str) -> std::path::PathBuf {
		let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
		let dir = std::env::temp_dir().join(format!("ldk_server_test_{}_{}", name, id));
		let _ = fs::remove_dir_all(&dir);
		fs::create_dir_all(&dir).unwrap();
		dir
	}

	#[test]
	fn test_compute_key_id() {
		let key_hex = "a".repeat(64);
		let key_id = compute_key_id(&key_hex);
		// Should be 16 hex chars
		assert_eq!(key_id.len(), 16);
		assert!(key_id.chars().all(|c| c.is_ascii_hexdigit()));
	}

	#[test]
	fn test_compute_key_id_deterministic() {
		let key_hex = "b".repeat(64);
		assert_eq!(compute_key_id(&key_hex), compute_key_id(&key_hex));
	}

	#[test]
	fn test_compute_key_id_different_keys() {
		let key_a = "a".repeat(64);
		let key_b = "b".repeat(64);
		assert_ne!(compute_key_id(&key_a), compute_key_id(&key_b));
	}

	#[test]
	fn test_atomic_write() {
		let dir = test_dir("atomic_write");
		let path = dir.join("test_file");
		atomic_write(&path, b"hello").unwrap();
		assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
		let perms = std::fs::metadata(&path).unwrap().permissions().mode();
		assert_eq!(perms & 0o777, 0o400);
	}

	#[test]
	fn test_load_from_dir_empty() {
		let dir = test_dir("load_empty");
		let store = ApiKeyStore::load_from_dir(&dir).unwrap();
		assert!(store.keys.is_empty());
	}

	#[test]
	fn test_load_from_dir_with_key() {
		let dir = test_dir("load_with_key");
		let key_hex = "ab".repeat(32);
		let toml_contents = format!("key = \"{key_hex}\"\nendpoints = [\"GetNodeInfo\"]\n");
		std::fs::write(dir.join("test.toml"), &toml_contents).unwrap();

		let store = ApiKeyStore::load_from_dir(&dir).unwrap();
		assert_eq!(store.keys.len(), 1);
		let key_id = compute_key_id(&key_hex);
		let (stored_key, endpoints) = store.keys.get(&key_id).unwrap();
		assert_eq!(stored_key, &key_hex);
		assert!(endpoints.contains("GetNodeInfo"));
	}

	#[test]
	fn test_load_from_dir_admin_key() {
		let dir = test_dir("load_admin");
		let key_hex = "cd".repeat(32);
		let toml_contents = format!("key = \"{key_hex}\"\nendpoints = [\"*\"]\n");
		std::fs::write(dir.join("admin.toml"), &toml_contents).unwrap();

		let store = ApiKeyStore::load_from_dir(&dir).unwrap();
		assert_eq!(store.keys.len(), 1);
		let key_id = compute_key_id(&key_hex);
		let (_, endpoints) = store.keys.get(&key_id).unwrap();
		assert!(endpoints.contains("*"));
	}

	#[test]
	fn test_load_from_dir_invalid_key_length() {
		let dir = test_dir("invalid_len");
		let toml_contents = "key = \"tooshort\"\nendpoints = []\n";
		std::fs::write(dir.join("bad.toml"), toml_contents).unwrap();

		let result = ApiKeyStore::load_from_dir(&dir);
		assert!(result.is_err());
	}

	#[test]
	fn test_init_creates_admin_key() {
		let dir = test_dir("init_creates");
		let api_keys_dir = ApiKeyStore::init(&dir).unwrap();
		assert!(api_keys_dir.join("admin.toml").exists());
	}

	#[test]
	fn test_init_migrates_legacy_key() {
		let dir = test_dir("init_migrates");
		let legacy_bytes: [u8; 32] = [0xab; 32];
		std::fs::write(dir.join("api_key"), legacy_bytes).unwrap();

		let api_keys_dir = ApiKeyStore::init(&dir).unwrap();
		let admin_toml = std::fs::read_to_string(api_keys_dir.join("admin.toml")).unwrap();
		assert!(admin_toml.contains("key = \""));
		assert!(admin_toml.contains("abababab")); // first few bytes hex
	}

	#[test]
	fn test_init_does_not_overwrite_existing_admin() {
		let dir = test_dir("init_no_overwrite");
		let api_keys_dir_path = dir.join("api_keys");
		std::fs::create_dir_all(&api_keys_dir_path).unwrap();
		let key_hex = "ff".repeat(32);
		let toml_contents = format!("key = \"{key_hex}\"\nendpoints = [\"*\"]\n");
		std::fs::write(api_keys_dir_path.join("admin.toml"), &toml_contents).unwrap();

		let api_keys_dir = ApiKeyStore::init(&dir).unwrap();
		let admin_toml = std::fs::read_to_string(api_keys_dir.join("admin.toml")).unwrap();
		assert!(admin_toml.contains(&key_hex));
	}

	#[test]
	fn test_validate_and_authorize_admin() {
		use ldk_node::bitcoin::hashes::hmac::{Hmac, HmacEngine};
		use ldk_node::bitcoin::hashes::HashEngine;

		let key_hex = "ab".repeat(32);
		let key_id = compute_key_id(&key_hex);
		let mut keys = HashMap::new();
		keys.insert(key_id.clone(), (key_hex.clone(), HashSet::from(["*".to_string()])));

		let store = ApiKeyStore { keys, api_keys_dir: PathBuf::new() };

		let timestamp =
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
		let body = b"test body";

		let mut hmac_engine: HmacEngine<sha256::Hash> = HmacEngine::new(key_hex.as_bytes());
		hmac_engine.input(&timestamp.to_be_bytes());
		hmac_engine.input(body);
		let hmac = Hmac::<sha256::Hash>::from_engine(hmac_engine);

		let result = store.validate_and_authorize(
			"GetNodeInfo",
			&key_id,
			timestamp,
			&hmac.to_string(),
			body,
		);
		assert!(result.is_ok());
		assert!(result.unwrap().contains("*"));
	}

	#[test]
	fn test_validate_and_authorize_restricted_allowed() {
		use ldk_node::bitcoin::hashes::hmac::{Hmac, HmacEngine};
		use ldk_node::bitcoin::hashes::HashEngine;

		let key_hex = "cd".repeat(32);
		let key_id = compute_key_id(&key_hex);
		let mut endpoints = HashSet::new();
		endpoints.insert("GetNodeInfo".to_string());
		let mut keys = HashMap::new();
		keys.insert(key_id.clone(), (key_hex.clone(), endpoints));

		let store = ApiKeyStore { keys, api_keys_dir: PathBuf::new() };

		let timestamp =
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
		let body = b"";

		let mut hmac_engine: HmacEngine<sha256::Hash> = HmacEngine::new(key_hex.as_bytes());
		hmac_engine.input(&timestamp.to_be_bytes());
		hmac_engine.input(body);
		let hmac = Hmac::<sha256::Hash>::from_engine(hmac_engine);

		let result = store.validate_and_authorize(
			"GetNodeInfo",
			&key_id,
			timestamp,
			&hmac.to_string(),
			body,
		);
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_and_authorize_restricted_denied() {
		use ldk_node::bitcoin::hashes::hmac::{Hmac, HmacEngine};
		use ldk_node::bitcoin::hashes::HashEngine;

		let key_hex = "cd".repeat(32);
		let key_id = compute_key_id(&key_hex);
		let mut endpoints = HashSet::new();
		endpoints.insert("GetNodeInfo".to_string());
		let mut keys = HashMap::new();
		keys.insert(key_id.clone(), (key_hex.clone(), endpoints));

		let store = ApiKeyStore { keys, api_keys_dir: PathBuf::new() };

		let timestamp =
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
		let body = b"";

		let mut hmac_engine: HmacEngine<sha256::Hash> = HmacEngine::new(key_hex.as_bytes());
		hmac_engine.input(&timestamp.to_be_bytes());
		hmac_engine.input(body);
		let hmac = Hmac::<sha256::Hash>::from_engine(hmac_engine);

		let result = store.validate_and_authorize(
			"OnchainSend",
			&key_id,
			timestamp,
			&hmac.to_string(),
			body,
		);
		assert!(result.is_err());
	}

	#[test]
	fn test_validate_and_authorize_get_permissions_always_allowed() {
		use ldk_node::bitcoin::hashes::hmac::{Hmac, HmacEngine};
		use ldk_node::bitcoin::hashes::HashEngine;

		let key_hex = "cd".repeat(32);
		let key_id = compute_key_id(&key_hex);
		let mut endpoints = HashSet::new();
		endpoints.insert("GetNodeInfo".to_string());
		let mut keys = HashMap::new();
		keys.insert(key_id.clone(), (key_hex.clone(), endpoints));

		let store = ApiKeyStore { keys, api_keys_dir: PathBuf::new() };

		let timestamp =
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
		let body = b"";

		let mut hmac_engine: HmacEngine<sha256::Hash> = HmacEngine::new(key_hex.as_bytes());
		hmac_engine.input(&timestamp.to_be_bytes());
		hmac_engine.input(body);
		let hmac = Hmac::<sha256::Hash>::from_engine(hmac_engine);

		let result = store.validate_and_authorize(
			"GetPermissions",
			&key_id,
			timestamp,
			&hmac.to_string(),
			body,
		);
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_and_authorize_invalid_key_id() {
		let store = ApiKeyStore { keys: HashMap::new(), api_keys_dir: PathBuf::new() };

		let result = store.validate_and_authorize(
			"GetNodeInfo",
			"0000000000000000",
			0,
			&"00".repeat(32),
			b"",
		);
		assert!(result.is_err());
	}

	#[test]
	fn test_validate_and_authorize_expired_timestamp() {
		use ldk_node::bitcoin::hashes::hmac::{Hmac, HmacEngine};
		use ldk_node::bitcoin::hashes::HashEngine;

		let key_hex = "ab".repeat(32);
		let key_id = compute_key_id(&key_hex);
		let mut keys = HashMap::new();
		keys.insert(key_id.clone(), (key_hex.clone(), HashSet::new()));

		let store = ApiKeyStore { keys, api_keys_dir: PathBuf::new() };

		let timestamp =
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
				- 600; // 10 minutes ago
		let body = b"";

		let mut hmac_engine: HmacEngine<sha256::Hash> = HmacEngine::new(key_hex.as_bytes());
		hmac_engine.input(&timestamp.to_be_bytes());
		hmac_engine.input(body);
		let hmac = Hmac::<sha256::Hash>::from_engine(hmac_engine);

		let result = store.validate_and_authorize(
			"GetNodeInfo",
			&key_id,
			timestamp,
			&hmac.to_string(),
			body,
		);
		assert!(result.is_err());
	}

	#[test]
	fn test_create_key() {
		let dir = test_dir("create_key");
		let mut store = ApiKeyStore { keys: HashMap::new(), api_keys_dir: dir.to_path_buf() };

		let key_hex = store.create_key("test-key", vec!["GetNodeInfo".to_string()]).unwrap();
		assert_eq!(key_hex.len(), 64);
		assert!(key_hex.chars().all(|c| c.is_ascii_hexdigit()));

		// Check file was written
		assert!(dir.join("test-key.toml").exists());

		// Check in-memory store updated
		let key_id = compute_key_id(&key_hex);
		assert!(store.keys.contains_key(&key_id));
	}

	#[test]
	fn test_create_key_invalid_name() {
		let dir = test_dir("invalid_name");
		let mut store = ApiKeyStore { keys: HashMap::new(), api_keys_dir: dir.to_path_buf() };

		let ep = vec!["GetNodeInfo".to_string()];

		// Empty name
		assert!(store.create_key("", ep.clone()).is_err());
		// Spaces
		assert!(store.create_key("bad name", ep.clone()).is_err());
		// Path traversal
		assert!(store.create_key("../etc/passwd", ep.clone()).is_err());
		assert!(store.create_key("bad/name", ep.clone()).is_err());
		// Leading dot (hidden files / temp file collision)
		assert!(store.create_key(".hidden", ep.clone()).is_err());
		assert!(store.create_key(".tmp_123", ep.clone()).is_err());
		// Dots in general (could fake file extensions)
		assert!(store.create_key("foo.toml", ep.clone()).is_err());
		assert!(store.create_key("key.bak", ep.clone()).is_err());
		// Wildcard / glob characters
		assert!(store.create_key("*", ep.clone()).is_err());
		assert!(store.create_key("key*", ep.clone()).is_err());
		// Null byte
		assert!(store.create_key("key\0name", ep.clone()).is_err());
		// Valid names should work
		assert!(store.create_key("good-name", ep.clone()).is_ok());
		assert!(store.create_key("good_name_2", ep).is_ok());
	}

	#[test]
	fn test_create_key_duplicate() {
		let dir = test_dir("duplicate");
		let mut store = ApiKeyStore { keys: HashMap::new(), api_keys_dir: dir.to_path_buf() };

		store.create_key("my-key", vec!["GetNodeInfo".to_string()]).unwrap();
		// Duplicate should fail
		assert!(store.create_key("my-key", vec!["GetNodeInfo".to_string()]).is_err());
	}

	#[test]
	fn test_validate_and_authorize_tampered_body() {
		use ldk_node::bitcoin::hashes::hmac::{Hmac, HmacEngine};
		use ldk_node::bitcoin::hashes::HashEngine;

		let key_hex = "ab".repeat(32);
		let key_id = compute_key_id(&key_hex);
		let mut keys = HashMap::new();
		keys.insert(key_id.clone(), (key_hex.clone(), HashSet::from(["*".to_string()])));
		let store = ApiKeyStore { keys, api_keys_dir: PathBuf::new() };

		let timestamp =
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
		let original_body = b"original body";
		let tampered_body = b"tampered body";

		let mut hmac_engine: HmacEngine<sha256::Hash> = HmacEngine::new(key_hex.as_bytes());
		hmac_engine.input(&timestamp.to_be_bytes());
		hmac_engine.input(original_body);
		let hmac = Hmac::<sha256::Hash>::from_engine(hmac_engine);

		// Valid key_id but body was tampered — should fail
		let result = store.validate_and_authorize(
			"GetNodeInfo",
			&key_id,
			timestamp,
			&hmac.to_string(),
			tampered_body,
		);
		assert!(result.is_err());
	}

	#[test]
	fn test_load_from_dir_multiple_keys() {
		let dir = test_dir("load_multiple");
		let admin_key = "aa".repeat(32);
		std::fs::write(
			dir.join("admin.toml"),
			format!("key = \"{admin_key}\"\nendpoints = [\"*\"]\n"),
		)
		.unwrap();
		let readonly_key = "bb".repeat(32);
		std::fs::write(
			dir.join("readonly.toml"),
			format!("key = \"{readonly_key}\"\nendpoints = [\"GetNodeInfo\"]\n"),
		)
		.unwrap();
		// Non-toml file should be ignored
		std::fs::write(dir.join("README.txt"), "not a key").unwrap();

		let store = ApiKeyStore::load_from_dir(&dir).unwrap();
		assert_eq!(store.keys.len(), 2);

		let admin_id = compute_key_id(&admin_key);
		let readonly_id = compute_key_id(&readonly_key);
		assert!(store.keys.get(&admin_id).unwrap().1.contains("*"));
		assert!(store.keys.get(&readonly_id).unwrap().1.contains("GetNodeInfo"));
	}

	#[test]
	fn test_create_key_empty_endpoints_rejected() {
		let dir = test_dir("empty_endpoints");
		let mut store = ApiKeyStore { keys: HashMap::new(), api_keys_dir: dir.to_path_buf() };

		let result = store.create_key("test-key", vec![]);
		assert!(result.is_err());
	}

	#[test]
	fn test_create_key_invalid_endpoint_rejected() {
		let dir = test_dir("invalid_endpoint");
		let mut store = ApiKeyStore { keys: HashMap::new(), api_keys_dir: dir.to_path_buf() };

		// Unknown endpoint should fail
		let result = store.create_key("test-key", vec!["FakeEndpoint".to_string()]);
		assert!(result.is_err());
		assert!(result.unwrap_err().message.contains("Unknown endpoint"));

		// Typo in endpoint name should fail
		let result = store.create_key("test-key", vec!["GetNodeinfo".to_string()]);
		assert!(result.is_err());

		// Valid endpoint should work
		let result = store.create_key("test-key", vec!["GetNodeInfo".to_string()]);
		assert!(result.is_ok());

		// Wildcard should work
		let result = store.create_key("admin-key", vec!["*".to_string()]);
		assert!(result.is_ok());
	}
}
