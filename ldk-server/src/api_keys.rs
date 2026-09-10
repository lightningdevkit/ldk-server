// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use hex::DisplayHex;
use ldk_node::bitcoin::hashes::hmac::{Hmac, HmacEngine};
use ldk_node::bitcoin::hashes::{sha256, Hash, HashEngine};
use ldk_server_grpc::endpoints::{
	BOLT11_CLAIM_FOR_ID_PATH, BOLT11_FAIL_FOR_ID_PATH, BOLT11_RECEIVE_FOR_HASH_PATH,
	BOLT11_RECEIVE_PATH, BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH,
	BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH, BOLT11_SEND_PATH, BOLT11_SEND_UNDERPAYING_PATH,
	BOLT12_CREATE_PAYER_PROOF_PATH, BOLT12_RECEIVE_PATH, BOLT12_RECEIVE_REFUND_PATH,
	BOLT12_SEND_PATH, BOLT12_SEND_REFUND_PATH, CLOSE_CHANNEL_PATH, CONNECT_PEER_PATH,
	CREATE_API_KEY_PATH, DECODE_INVOICE_PATH, DECODE_OFFER_PATH, DISCONNECT_PEER_PATH,
	EXPORT_PATHFINDING_SCORES_PATH, FORCE_CLOSE_CHANNEL_PATH, GET_BALANCES_PATH,
	GET_NODE_INFO_PATH, GET_PAYMENT_DETAILS_PATH, GET_PERMISSIONS_PATH, GRAPH_GET_CHANNEL_PATH,
	GRAPH_GET_NODE_PATH, GRAPH_LIST_CHANNELS_PATH, GRAPH_LIST_NODES_PATH, LIST_API_KEYS_PATH,
	LIST_CHANNELS_PATH, LIST_FORWARDED_PAYMENTS_PATH, LIST_PAYMENTS_PATH, LIST_PEERS_PATH,
	ONCHAIN_RECEIVE_PATH, ONCHAIN_SEND_PATH, OPEN_CHANNEL_PATH, REVOKE_API_KEY_PATH,
	SIGN_MESSAGE_PATH, SPLICE_IN_PATH, SPLICE_OUT_PATH, SPONTANEOUS_SEND_PATH,
	SUBSCRIBE_EVENTS_PATH, UNIFIED_SEND_PATH, UPDATE_CHANNEL_CONFIG_PATH, VERIFY_SIGNATURE_PATH,
};
use ldk_server_grpc::permissions::{
	ADMIN_PERMISSION, ALL_PERMISSIONS, API_KEYS_MANAGE_PERMISSION, CHANNELS_FORCE_CLOSE_PERMISSION,
	CHANNELS_MANAGE_PERMISSION, CHANNELS_READ_PERMISSION, CHANNELS_SPLICE_PERMISSION,
	EVENTS_READ_PERMISSION, GRAPH_READ_PERMISSION, INVOICES_CREATE_PERMISSION,
	MESSAGES_SIGN_PERMISSION, MESSAGES_VERIFY_PERMISSION, NODE_READ_PERMISSION,
	ONCHAIN_RECEIVE_PERMISSION, ONCHAIN_SEND_PERMISSION, PAYMENTS_CLAIM_PERMISSION,
	PAYMENTS_READ_PERMISSION, PAYMENTS_SEND_PERMISSION, PEERS_MANAGE_PERMISSION,
	PEERS_READ_PERMISSION, UTILITIES_READ_PERMISSION,
};
use serde::Deserialize;

use crate::api::error::{LdkServerError, LdkServerErrorCode};
use crate::util::{create_dir_all_private, read_to_string_with_limit, write_new};

const API_KEY_FILE_SIZE_LIMIT: usize = 4096;
const API_KEYS_DIR: &str = "api_keys";
const ADMIN_KEY_FILE: &str = "admin.toml";
const AUTH_TIMESTAMP_TOLERANCE_SECS: u64 = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ApiKeyInfo {
	pub(crate) id: String,
	pub(crate) name: String,
	pub(crate) permissions: BTreeSet<String>,
}

impl ApiKeyInfo {
	pub(crate) fn is_admin(&self) -> bool {
		self.permissions.contains(ADMIN_PERMISSION)
	}

	pub(crate) fn allows(&self, permission: &str) -> bool {
		self.is_admin() || self.permissions.contains(permission)
	}
}

#[derive(Debug)]
pub(crate) struct CreatedApiKey {
	pub(crate) info: ApiKeyInfo,
	pub(crate) secret: String,
}

#[derive(Debug)]
struct ApiKeyRecord {
	info: Arc<ApiKeyInfo>,
	secret: String,
	path: PathBuf,
}

#[derive(Deserialize)]
struct StoredApiKey {
	id: String,
	name: String,
	key: String,
	permissions: Vec<String>,
}

pub(crate) struct ApiKeyStore {
	keys: RwLock<HashMap<String, Arc<ApiKeyRecord>>>,
	management: Mutex<()>,
	directory: PathBuf,
}

impl ApiKeyStore {
	pub(crate) fn load_or_create(storage_dir: &Path) -> io::Result<Self> {
		let directory = storage_dir.join(API_KEYS_DIR);
		create_dir_all_private(&directory)?;
		fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;

		let mut store =
			Self { keys: RwLock::new(HashMap::new()), management: Mutex::new(()), directory };
		store.load_key_files()?;
		if store
			.keys
			.get_mut()
			.map_err(|_| io::Error::other("API key store lock is poisoned"))?
			.is_empty()
		{
			store.create_initial_admin()?;
		}
		Ok(store)
	}

	fn load_key_files(&mut self) -> io::Result<()> {
		let keys =
			self.keys.get_mut().map_err(|_| io::Error::other("API key store lock is poisoned"))?;
		for entry in fs::read_dir(&self.directory)? {
			let path = entry?.path();
			if path.extension().is_none_or(|extension| extension != "toml") {
				continue;
			}

			let contents = read_to_string_with_limit(&path, API_KEY_FILE_SIZE_LIMIT)?;
			let stored: StoredApiKey = toml::from_str(&contents).map_err(|error| {
				invalid_data(format!("Failed to parse API key file {}: {error}", path.display()))
			})?;
			let record = record_from_stored(stored, path)?;
			if keys.values().any(|existing| existing.info.name == record.info.name) {
				return Err(invalid_data(format!("Duplicate API key name: {}", record.info.name)));
			}
			if keys.insert(record.info.id.clone(), Arc::new(record)).is_some() {
				return Err(invalid_data("Duplicate API key ID"));
			}
		}
		Ok(())
	}

	fn create_initial_admin(&mut self) -> io::Result<()> {
		let secret = generate_secret()?;
		let info = ApiKeyInfo {
			id: compute_key_id(&secret),
			name: "admin".to_string(),
			permissions: BTreeSet::from([ADMIN_PERMISSION.to_string()]),
		};
		let path = self.directory.join(ADMIN_KEY_FILE);
		write_key_file(&path, &info, &secret)?;
		self.keys
			.get_mut()
			.map_err(|_| io::Error::other("API key store lock is poisoned"))?
			.insert(info.id.clone(), Arc::new(ApiKeyRecord { info: Arc::new(info), secret, path }));

		Ok(())
	}

	pub(crate) fn authenticate(
		&self, method: &str, auth_header: Option<&str>, body: &[u8],
	) -> Result<Arc<ApiKeyInfo>, LdkServerError> {
		let auth_error = |message| LdkServerError::new(LdkServerErrorCode::AuthError, message);
		let auth_header = auth_header.ok_or_else(|| auth_error("Missing x-auth metadata"))?;
		let auth_data =
			auth_header.strip_prefix("HMAC ").ok_or_else(|| auth_error("Invalid x-auth format"))?;
		let mut parts = auth_data.split(':');
		let key_id = parts.next().ok_or_else(|| auth_error("Invalid x-auth format"))?;
		let timestamp = parts
			.next()
			.ok_or_else(|| auth_error("Invalid x-auth format"))?
			.parse::<u64>()
			.map_err(|_| auth_error("Invalid timestamp"))?;
		let provided_hmac = parts.next().ok_or_else(|| auth_error("Invalid x-auth format"))?;
		if parts.next().is_some() || !is_hex(key_id, 32) {
			return Err(auth_error("Invalid x-auth format"));
		}

		let now = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map_err(|_| auth_error("System time error"))?
			.as_secs();
		if now.abs_diff(timestamp) > AUTH_TIMESTAMP_TOLERANCE_SECS {
			return Err(auth_error("Request timestamp expired"));
		}

		let record = self
			.keys
			.read()
			.map_err(|_| key_store_lock_error())?
			.get(key_id)
			.cloned()
			.ok_or_else(|| auth_error("Invalid credentials"))?;
		let expected_hmac = compute_auth_hmac(&record.secret, key_id, method, timestamp, body);
		let provided_hmac = provided_hmac
			.parse::<Hmac<sha256::Hash>>()
			.map_err(|_| auth_error("Invalid HMAC in x-auth"))?;
		if expected_hmac != provided_hmac {
			return Err(auth_error("Invalid credentials"));
		}

		Ok(Arc::clone(&record.info))
	}

	// Call management operations from a blocking thread. Authentication never takes this mutex.
	pub(crate) fn create_key(
		&self, name: &str, permissions: Vec<String>, issuer: &ApiKeyInfo,
	) -> Result<CreatedApiKey, LdkServerError> {
		self.create_key_with_writer(name, permissions, issuer, write_key_file)
	}

	fn create_key_with_writer(
		&self, name: &str, permissions: Vec<String>, issuer: &ApiKeyInfo,
		write: impl FnOnce(&Path, &ApiKeyInfo, &str) -> io::Result<()>,
	) -> Result<CreatedApiKey, LdkServerError> {
		validate_name(name)?;
		let permissions = validate_permissions(permissions).map_err(invalid_request)?;
		let _management = self.management.lock().map_err(|_| key_store_lock_error())?;
		let secret = generate_secret().map_err(internal_error)?;
		let info = ApiKeyInfo { id: compute_key_id(&secret), name: name.to_string(), permissions };
		{
			let keys = self.keys.read().map_err(|_| key_store_lock_error())?;
			if !keys.contains_key(&issuer.id) {
				return Err(LdkServerError::new(
					LdkServerErrorCode::AuthError,
					"Invalid credentials",
				));
			}
			if keys.values().any(|record| record.info.name == name) {
				return Err(invalid_request(format!("API key name already exists: {name}")));
			}
			if !issuer.is_admin()
				&& info
					.permissions
					.iter()
					.any(|permission| !issuer.permissions.contains(permission))
			{
				return Err(authorization_error(
					"Cannot grant a permission that the calling key does not have",
				));
			}
			if keys.contains_key(&info.id) {
				return Err(internal_error("Generated a duplicate API key ID"));
			}
		}
		let path = self.directory.join(format!("{}.toml", info.id));
		write(&path, &info, &secret).map_err(internal_error)?;
		let record =
			Arc::new(ApiKeyRecord { info: Arc::new(info.clone()), secret: secret.clone(), path });
		self.keys.write().map_err(|_| key_store_lock_error())?.insert(info.id.clone(), record);
		Ok(CreatedApiKey { info, secret })
	}

	pub(crate) fn list_keys(&self) -> Result<Vec<ApiKeyInfo>, LdkServerError> {
		let records = self.keys.read().map_err(|_| key_store_lock_error())?;
		let mut keys: Vec<_> = records.values().map(|record| (*record.info).clone()).collect();
		keys.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
		Ok(keys)
	}

	pub(crate) fn revoke_key(&self, id: &str, issuer: &ApiKeyInfo) -> Result<(), LdkServerError> {
		if !is_hex(id, 32) {
			return Err(invalid_request(
				"API key ID must contain exactly 32 hexadecimal characters",
			));
		}
		let _management = self.management.lock().map_err(|_| key_store_lock_error())?;
		let keys = self.keys.read().map_err(|_| key_store_lock_error())?;
		if !keys.contains_key(&issuer.id) {
			return Err(LdkServerError::new(LdkServerErrorCode::AuthError, "Invalid credentials"));
		}
		let record =
			keys.get(id).ok_or_else(|| invalid_request(format!("Unknown API key ID: {id}")))?;
		if !issuer.is_admin()
			&& (record.info.is_admin()
				|| record
					.info
					.permissions
					.iter()
					.any(|permission| !issuer.permissions.contains(permission)))
		{
			return Err(authorization_error(
				"Cannot revoke a key with permissions that the calling key does not have",
			));
		}
		if record.info.is_admin()
			&& keys.values().filter(|record| record.info.is_admin()).count() == 1
		{
			return Err(invalid_request("Cannot revoke the final admin API key"));
		}

		let path = record.path.clone();
		drop(keys);
		match fs::remove_file(path) {
			Ok(()) => {},
			// The file may have been deleted manually; still revoke the key from memory.
			Err(error) if error.kind() == io::ErrorKind::NotFound => {},
			Err(error) => return Err(internal_error(error)),
		}
		self.keys.write().map_err(|_| key_store_lock_error())?.remove(id);
		File::open(&self.directory)
			.and_then(|directory| directory.sync_all())
			.map_err(internal_error)?;
		Ok(())
	}
}

pub(crate) fn compute_key_id(secret: &str) -> String {
	let hash = sha256::Hash::hash(secret.as_bytes());
	hash[..16].to_lower_hex_string()
}

pub(crate) enum MethodAuthorization {
	Permission(&'static str),
	AuthenticatedOnly,
	Unknown,
}

pub(crate) fn method_authorization(method: &str) -> MethodAuthorization {
	match method {
		GET_NODE_INFO_PATH | GET_BALANCES_PATH | EXPORT_PATHFINDING_SCORES_PATH => {
			MethodAuthorization::Permission(NODE_READ_PERMISSION)
		},
		ONCHAIN_RECEIVE_PATH => MethodAuthorization::Permission(ONCHAIN_RECEIVE_PERMISSION),
		ONCHAIN_SEND_PATH => MethodAuthorization::Permission(ONCHAIN_SEND_PERMISSION),
		BOLT11_RECEIVE_PATH
		| BOLT11_RECEIVE_FOR_HASH_PATH
		| BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH
		| BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH
		| BOLT12_RECEIVE_PATH
		| BOLT12_RECEIVE_REFUND_PATH => MethodAuthorization::Permission(INVOICES_CREATE_PERMISSION),
		BOLT11_CLAIM_FOR_ID_PATH | BOLT11_FAIL_FOR_ID_PATH => {
			MethodAuthorization::Permission(PAYMENTS_CLAIM_PERMISSION)
		},
		BOLT11_SEND_PATH
		| BOLT11_SEND_UNDERPAYING_PATH
		| BOLT12_SEND_PATH
		| BOLT12_SEND_REFUND_PATH
		| SPONTANEOUS_SEND_PATH
		| UNIFIED_SEND_PATH => MethodAuthorization::Permission(PAYMENTS_SEND_PERMISSION),
		GET_PAYMENT_DETAILS_PATH | LIST_PAYMENTS_PATH | LIST_FORWARDED_PAYMENTS_PATH => {
			MethodAuthorization::Permission(PAYMENTS_READ_PERMISSION)
		},
		LIST_CHANNELS_PATH => MethodAuthorization::Permission(CHANNELS_READ_PERMISSION),
		SPLICE_IN_PATH | SPLICE_OUT_PATH => {
			MethodAuthorization::Permission(CHANNELS_SPLICE_PERMISSION)
		},
		OPEN_CHANNEL_PATH | UPDATE_CHANNEL_CONFIG_PATH | CLOSE_CHANNEL_PATH => {
			MethodAuthorization::Permission(CHANNELS_MANAGE_PERMISSION)
		},
		FORCE_CLOSE_CHANNEL_PATH => {
			MethodAuthorization::Permission(CHANNELS_FORCE_CLOSE_PERMISSION)
		},
		LIST_PEERS_PATH => MethodAuthorization::Permission(PEERS_READ_PERMISSION),
		CONNECT_PEER_PATH | DISCONNECT_PEER_PATH => {
			MethodAuthorization::Permission(PEERS_MANAGE_PERMISSION)
		},
		SIGN_MESSAGE_PATH | BOLT12_CREATE_PAYER_PROOF_PATH => {
			MethodAuthorization::Permission(MESSAGES_SIGN_PERMISSION)
		},
		VERIFY_SIGNATURE_PATH => MethodAuthorization::Permission(MESSAGES_VERIFY_PERMISSION),
		GRAPH_LIST_CHANNELS_PATH
		| GRAPH_GET_CHANNEL_PATH
		| GRAPH_LIST_NODES_PATH
		| GRAPH_GET_NODE_PATH => MethodAuthorization::Permission(GRAPH_READ_PERMISSION),
		DECODE_INVOICE_PATH | DECODE_OFFER_PATH => {
			MethodAuthorization::Permission(UTILITIES_READ_PERMISSION)
		},
		SUBSCRIBE_EVENTS_PATH => MethodAuthorization::Permission(EVENTS_READ_PERMISSION),
		CREATE_API_KEY_PATH | LIST_API_KEYS_PATH | REVOKE_API_KEY_PATH => {
			MethodAuthorization::Permission(API_KEYS_MANAGE_PERMISSION)
		},
		GET_PERMISSIONS_PATH => MethodAuthorization::AuthenticatedOnly,
		_ => MethodAuthorization::Unknown,
	}
}

pub(crate) fn compute_auth_hmac(
	secret: &str, key_id: &str, method: &str, timestamp: u64, body: &[u8],
) -> Hmac<sha256::Hash> {
	let mut engine = HmacEngine::new(secret.as_bytes());
	ldk_server_grpc::auth::write_auth_preimage(key_id, method, timestamp, body, |part| {
		engine.input(part)
	});
	Hmac::from_engine(engine)
}

fn record_from_stored(stored: StoredApiKey, path: PathBuf) -> io::Result<ApiKeyRecord> {
	if !is_hex(&stored.key, 64) {
		return Err(invalid_data(format!("Invalid API key in {}", path.display())));
	}
	if !is_hex(&stored.id, 32) || stored.id != compute_key_id(&stored.key) {
		return Err(invalid_data(format!("Invalid API key ID in {}", path.display())));
	}
	validate_name_value(&stored.name).map_err(invalid_data)?;
	let permissions = validate_permissions(stored.permissions).map_err(invalid_data)?;
	Ok(ApiKeyRecord {
		info: Arc::new(ApiKeyInfo { id: stored.id, name: stored.name, permissions }),
		secret: stored.key,
		path,
	})
}

fn validate_name(name: &str) -> Result<(), LdkServerError> {
	validate_name_value(name).map_err(invalid_request)
}

fn validate_name_value(name: &str) -> Result<(), String> {
	if name.is_empty()
		|| name.len() > 64
		|| !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
	{
		return Err(
			"API key name must contain 1 to 64 ASCII letters, numbers, hyphens, or underscores"
				.to_string(),
		);
	}
	Ok(())
}

fn validate_permissions(permissions: Vec<String>) -> Result<BTreeSet<String>, String> {
	let permissions: BTreeSet<_> = permissions.into_iter().collect();
	if permissions.is_empty() {
		return Err("At least one API key permission is required".to_string());
	}
	for permission in &permissions {
		if !ALL_PERMISSIONS.contains(&permission.as_str()) {
			return Err(format!("Unknown API key permission: {permission}"));
		}
	}
	if permissions.contains(ADMIN_PERMISSION) && permissions.len() != 1 {
		return Err("The admin permission must be used by itself".to_string());
	}
	Ok(permissions)
}

fn generate_secret() -> io::Result<String> {
	let mut bytes = [0u8; 32];
	getrandom::getrandom(&mut bytes).map_err(io::Error::other)?;
	Ok(bytes.to_lower_hex_string())
}

fn write_key_file(path: &Path, info: &ApiKeyInfo, secret: &str) -> io::Result<()> {
	let permissions = info
		.permissions
		.iter()
		.map(|permission| format!("\"{permission}\""))
		.collect::<Vec<_>>()
		.join(", ");
	let contents = format!(
		"id = \"{}\"\nname = \"{}\"\nkey = \"{}\"\npermissions = [{}]\n",
		info.id, info.name, secret, permissions
	);

	let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("api-key");
	let mut suffix = [0u8; 8];
	getrandom::getrandom(&mut suffix).map_err(io::Error::other)?;
	let temporary_path = path.with_file_name(format!(
		".{file_name}.{}.{}.tmp",
		std::process::id(),
		suffix.to_lower_hex_string()
	));
	let result = (|| {
		write_new(&temporary_path, contents.as_bytes(), 0o400)?;
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

fn is_hex(value: &str, expected_length: usize) -> bool {
	value.len() == expected_length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn invalid_data(message: impl Into<String>) -> io::Error {
	io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn invalid_request(message: impl Into<String>) -> LdkServerError {
	LdkServerError::new(LdkServerErrorCode::InvalidRequestError, message)
}

fn authorization_error(message: impl Into<String>) -> LdkServerError {
	LdkServerError::new(LdkServerErrorCode::AuthorizationError, message)
}

fn key_store_lock_error() -> LdkServerError {
	internal_error("API key store lock is poisoned")
}

fn internal_error(message: impl std::fmt::Display) -> LdkServerError {
	LdkServerError::new(LdkServerErrorCode::InternalServerError, message.to_string())
}

#[cfg(test)]
mod tests {
	use std::sync::atomic::{AtomicU32, Ordering};

	use ldk_server_grpc::endpoints::{GET_BALANCES_PATH, GET_NODE_INFO_PATH};
	use ldk_server_grpc::permissions::{API_KEYS_MANAGE_PERMISSION, NODE_READ_PERMISSION};

	use super::*;

	static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

	#[test]
	fn every_rpc_has_the_expected_authorization() {
		// Keep this contract independent of the production mapping. The schema comparison
		// requires each new RPC to have an explicit authorization expectation here.
		let expected = [
			("GetNodeInfo", Some("node:read")),
			("GetBalances", Some("node:read")),
			("OnchainReceive", Some("onchain:receive")),
			("OnchainSend", Some("onchain:send")),
			("Bolt11Receive", Some("invoices:create")),
			("Bolt11ReceiveForHash", Some("invoices:create")),
			("Bolt11ClaimForId", Some("payments:claim")),
			("Bolt11FailForId", Some("payments:claim")),
			("Bolt11ReceiveViaJitChannel", Some("invoices:create")),
			("Bolt11ReceiveVariableAmountViaJitChannel", Some("invoices:create")),
			("Bolt11Send", Some("payments:send")),
			("Bolt11SendUnderpaying", Some("payments:send")),
			("Bolt12Receive", Some("invoices:create")),
			("Bolt12Send", Some("payments:send")),
			("Bolt12SendRefund", Some("payments:send")),
			("Bolt12ReceiveRefund", Some("invoices:create")),
			("Bolt12CreatePayerProof", Some("messages:sign")),
			("SpontaneousSend", Some("payments:send")),
			("OpenChannel", Some("channels:manage")),
			("SpliceIn", Some("channels:splice")),
			("SpliceOut", Some("channels:splice")),
			("UpdateChannelConfig", Some("channels:manage")),
			("CloseChannel", Some("channels:manage")),
			("ForceCloseChannel", Some("channels:force_close")),
			("ListChannels", Some("channels:read")),
			("GetPaymentDetails", Some("payments:read")),
			("ListPayments", Some("payments:read")),
			("ListForwardedPayments", Some("payments:read")),
			("ConnectPeer", Some("peers:manage")),
			("DisconnectPeer", Some("peers:manage")),
			("ListPeers", Some("peers:read")),
			("SignMessage", Some("messages:sign")),
			("VerifySignature", Some("messages:verify")),
			("ExportPathfindingScores", Some("node:read")),
			("UnifiedSend", Some("payments:send")),
			("DecodeInvoice", Some("utilities:read")),
			("DecodeOffer", Some("utilities:read")),
			("GraphListChannels", Some("graph:read")),
			("GraphGetChannel", Some("graph:read")),
			("GraphListNodes", Some("graph:read")),
			("GraphGetNode", Some("graph:read")),
			("SubscribeEvents", Some("events:read")),
			("CreateApiKey", Some("api_keys:manage")),
			("ListApiKeys", Some("api_keys:manage")),
			("RevokeApiKey", Some("api_keys:manage")),
			("GetPermissions", None),
		];
		let declared_methods: BTreeSet<_> =
			include_str!("../../ldk-server-grpc/src/proto/api.proto")
				.lines()
				.filter_map(|line| {
					let mut words = line.split_whitespace();
					if words.next() != Some("rpc") {
						return None;
					}
					Some(words.next().expect("RPC name").split('(').next().unwrap())
				})
				.collect();
		let tested_methods: BTreeSet<_> = expected.iter().map(|(method, _)| *method).collect();
		assert_eq!(tested_methods.len(), expected.len(), "Duplicate RPC in permission table");
		assert_eq!(declared_methods, tested_methods, "Update the RPC permission test table");

		for (method, expected_permission) in expected {
			let required = match (method_authorization(method), expected_permission) {
				(MethodAuthorization::Permission(actual), Some(expected)) => {
					assert_eq!(actual, expected, "Incorrect permission for {method}");
					actual
				},
				(MethodAuthorization::AuthenticatedOnly, None) => continue,
				_ => panic!("Incorrect authorization classification for {method}"),
			};
			assert!(ALL_PERMISSIONS.contains(&required), "Unknown permission for {method}");
			let mut key = ApiKeyInfo {
				id: "test".to_string(),
				name: "test".to_string(),
				permissions: BTreeSet::new(),
			};
			assert!(!key.allows(required), "Key without permissions must not access {method}");
			for permission in ALL_PERMISSIONS {
				key.permissions = BTreeSet::from([permission.to_string()]);
				assert_eq!(
					key.allows(required),
					permission == "admin" || permission == required,
					"Unexpected access to {method} with {permission}"
				);
			}
		}
	}

	#[test]
	fn creates_initial_admin_key() {
		let directory = test_directory("initial-admin");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let keys = store.list_keys().unwrap();

		assert_eq!(keys.len(), 1);
		assert_eq!(keys[0].name, "admin");
		assert!(keys[0].is_admin());
		let admin_path = directory.join(API_KEYS_DIR).join(ADMIN_KEY_FILE);
		assert!(admin_path.exists());
		assert_eq!(fs::metadata(admin_path).unwrap().permissions().mode() & 0o777, 0o400);
		assert_eq!(
			fs::metadata(directory.join(API_KEYS_DIR)).unwrap().permissions().mode() & 0o777,
			0o700
		);

		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn load_api_key_rejects_oversized_toml() {
		let directory = test_directory("oversized-key-toml");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let path = directory.join(API_KEYS_DIR).join("oversized.toml");
		fs::write(&path, vec![b' '; API_KEY_FILE_SIZE_LIMIT + 1]).unwrap();
		drop(store);
		let error = ApiKeyStore::load_or_create(&directory).err().unwrap();
		assert_eq!(error.kind(), io::ErrorKind::InvalidData);
		assert!(error.to_string().contains("exceeds"));
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn load_rejects_duplicate_key_names_and_ids() {
		for duplicate in ["name", "ID"] {
			let directory = test_directory("duplicate-key");
			let store = ApiKeyStore::load_or_create(&directory).unwrap();
			let (mut info, mut secret) = {
				let keys = store.keys.read().unwrap();
				let admin = keys.values().next().unwrap();
				((*admin.info).clone(), admin.secret.clone())
			};
			if duplicate == "name" {
				// Same name, but a different valid secret and ID.
				secret = generate_secret().unwrap();
				info.id = compute_key_id(&secret);
			} else {
				// Same secret and ID, but a different valid name.
				info.name = "another-admin".to_string();
			}
			write_key_file(&directory.join(API_KEYS_DIR).join("duplicate.toml"), &info, &secret)
				.unwrap();
			drop(store);

			let error = ApiKeyStore::load_or_create(&directory).err().unwrap();
			assert_eq!(error.kind(), io::ErrorKind::InvalidData);
			assert!(error.to_string().contains(&format!("Duplicate API key {duplicate}")));
			fs::remove_dir_all(directory).unwrap();
		}
	}

	#[test]
	fn creates_lists_revokes_and_reloads_key() {
		let directory = test_directory("lifecycle");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let created =
			store.create_key("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();

		assert_eq!(store.list_keys().unwrap().len(), 2);
		assert!(created.info.allows(NODE_READ_PERMISSION));
		assert!(!created.info.is_admin());
		drop(store);

		let reloaded = ApiKeyStore::load_or_create(&directory).unwrap();
		assert_eq!(reloaded.list_keys().unwrap().len(), 2);
		reloaded.revoke_key(&created.info.id, &admin).unwrap();
		assert_eq!(reloaded.list_keys().unwrap(), vec![admin]);

		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn revokes_key_when_its_file_is_missing() {
		let directory = test_directory("revoke-missing-file");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let reader =
			store.create_key("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();
		let timestamp = now();
		let signature =
			compute_auth_hmac(&reader.secret, &reader.info.id, GET_NODE_INFO_PATH, timestamp, b"");
		let header = format!("HMAC {}:{timestamp}:{signature}", reader.info.id);
		fs::remove_file(directory.join(API_KEYS_DIR).join(format!("{}.toml", reader.info.id)))
			.unwrap();
		assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&header), b"").is_ok());

		store.revoke_key(&reader.info.id, &admin).unwrap();
		assert_eq!(
			store.authenticate(GET_NODE_INFO_PATH, Some(&header), b"").unwrap_err().error_code,
			LdkServerErrorCode::AuthError
		);
		assert_eq!(store.list_keys().unwrap(), vec![admin.clone()]);
		assert_eq!(
			ApiKeyStore::load_or_create(&directory).unwrap().list_keys().unwrap(),
			vec![admin]
		);
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn revoke_rejects_malformed_ids_without_echoing_them() {
		let directory = test_directory("revoke-invalid-id");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		for id in [String::new(), "a".repeat(31), "a".repeat(33), "z".repeat(32), "a".repeat(8192)]
		{
			let error = store.revoke_key(&id, &admin).unwrap_err();
			assert_eq!(error.error_code, LdkServerErrorCode::InvalidRequestError);
			assert_eq!(error.message, "API key ID must contain exactly 32 hexadecimal characters");
		}
		assert_eq!(store.list_keys().unwrap(), vec![admin]);
		fs::remove_dir_all(directory).unwrap();
	}

	#[tokio::test]
	async fn authentication_continues_during_key_file_write() {
		use std::time::Duration;
		let directory = test_directory("slow-key-write");
		let store = Arc::new(ApiKeyStore::load_or_create(&directory).unwrap());
		let admin = store.list_keys().unwrap().remove(0);
		let reader =
			store.create_key("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();
		let timestamp = now();
		let signature =
			compute_auth_hmac(&reader.secret, &reader.info.id, GET_NODE_INFO_PATH, timestamp, b"");
		let header = format!("HMAC {}:{timestamp}:{signature}", reader.info.id);
		let first = store.authenticate(GET_NODE_INFO_PATH, Some(&header), b"").unwrap();
		let (started_tx, started_rx) = tokio::sync::oneshot::channel();
		let (release_tx, release_rx) = std::sync::mpsc::channel();
		let writer_store = Arc::clone(&store);
		let writer = tokio::task::spawn_blocking(move || {
			writer_store.create_key_with_writer(
				"pending",
				vec![NODE_READ_PERMISSION.to_string()],
				&admin,
				|path, info, secret| {
					started_tx.send(()).unwrap();
					release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
					write_key_file(path, info, secret)
				},
			)
		});
		started_rx.await.unwrap();
		let auth_store = Arc::clone(&store);
		let auth = tokio::task::spawn_blocking(move || {
			let key = auth_store.authenticate(GET_NODE_INFO_PATH, Some(&header), b"").unwrap();
			assert!(!auth_store.list_keys().unwrap().iter().any(|key| key.name == "pending"));
			key
		});
		let result = tokio::time::timeout(Duration::from_secs(1), auth).await;
		// Release the writer even if authentication timed out, so the test cannot hang.
		release_tx.send(()).unwrap();
		writer.await.unwrap().unwrap();
		let second = result.expect("Authentication waited for disk I/O").unwrap();
		assert!(Arc::ptr_eq(&first, &second));
		assert!(store.list_keys().unwrap().iter().any(|key| key.name == "pending"));
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn failed_key_write_does_not_publish_key() {
		let directory = test_directory("failed-key-write");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let error = store
			.create_key_with_writer(
				"reader",
				vec![NODE_READ_PERMISSION.to_string()],
				&admin,
				|_, _, _| Err(io::Error::other("injected write failure")),
			)
			.unwrap_err();
		assert_eq!(error.error_code, LdkServerErrorCode::InternalServerError);
		assert_eq!(store.list_keys().unwrap(), vec![admin.clone()]);
		store.create_key("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn revoked_issuer_cannot_manage_keys_with_an_old_snapshot() {
		let directory = test_directory("revoked-issuer");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let manager = store
			.create_key(
				"manager",
				vec![API_KEYS_MANAGE_PERMISSION.to_string(), NODE_READ_PERMISSION.to_string()],
				&admin,
			)
			.unwrap()
			.info;
		let reader =
			store.create_key("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();
		store.revoke_key(&manager.id, &admin).unwrap();
		assert_eq!(
			store
				.create_key("late", vec![NODE_READ_PERMISSION.to_string()], &manager)
				.unwrap_err()
				.error_code,
			LdkServerErrorCode::AuthError
		);
		assert_eq!(
			store.revoke_key(&reader.info.id, &manager).unwrap_err().error_code,
			LdkServerErrorCode::AuthError
		);
		assert!(store.list_keys().unwrap().contains(&reader.info));
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn concurrent_creates_keep_key_names_unique() {
		let directory = test_directory("concurrent-create");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let barrier = std::sync::Barrier::new(4);
		std::thread::scope(|scope| {
			let handles: Vec<_> = (0..4)
				.map(|_| {
					scope.spawn(|| {
						barrier.wait();
						store.create_key("reader", vec![NODE_READ_PERMISSION.to_string()], &admin)
					})
				})
				.collect();
			let results: Vec<_> =
				handles.into_iter().map(|handle| handle.join().unwrap()).collect();
			assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
			for error in results.into_iter().filter_map(Result::err) {
				assert_eq!(error.error_code, LdkServerErrorCode::InvalidRequestError);
			}
		});
		let reloaded = ApiKeyStore::load_or_create(&directory).unwrap();
		assert_eq!(store.list_keys().unwrap(), reloaded.list_keys().unwrap());
		assert_eq!(reloaded.list_keys().unwrap().len(), 2);
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn scoped_manager_cannot_escalate_or_revoke_admin() {
		let directory = test_directory("delegation");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let manager = store
			.create_key(
				"manager",
				vec![API_KEYS_MANAGE_PERMISSION.to_string(), NODE_READ_PERMISSION.to_string()],
				&admin,
			)
			.unwrap()
			.info;

		let delegated = store
			.create_key("delegated", vec![NODE_READ_PERMISSION.to_string()], &manager)
			.unwrap();
		assert!(delegated.info.allows(NODE_READ_PERMISSION));
		assert_eq!(
			store
				.create_key("escalated", vec![ADMIN_PERMISSION.to_string()], &manager)
				.unwrap_err()
				.error_code,
			LdkServerErrorCode::AuthorizationError
		);
		assert_eq!(
			store.revoke_key(&admin.id, &manager).unwrap_err().error_code,
			LdkServerErrorCode::AuthorizationError
		);

		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn scoped_manager_revokes_only_keys_within_its_permissions() {
		let directory = test_directory("scoped-revocation");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let manager = store
			.create_key(
				"manager",
				vec![API_KEYS_MANAGE_PERMISSION.to_string(), NODE_READ_PERMISSION.to_string()],
				&admin,
			)
			.unwrap()
			.info;
		let reader = store
			.create_key("reader", vec![NODE_READ_PERMISSION.to_string()], &admin)
			.unwrap()
			.info;
		let peer = store
			.create_key(
				"peer",
				vec![NODE_READ_PERMISSION.to_string(), PAYMENTS_SEND_PERMISSION.to_string()],
				&admin,
			)
			.unwrap()
			.info;

		assert_eq!(
			store.revoke_key(&peer.id, &manager).unwrap_err().error_code,
			LdkServerErrorCode::AuthorizationError
		);
		store.revoke_key(&reader.id, &manager).unwrap();
		let keys = store.list_keys().unwrap();
		assert!(keys.contains(&peer));
		assert!(!keys.contains(&reader));
		assert_eq!(ApiKeyStore::load_or_create(&directory).unwrap().list_keys().unwrap(), keys);
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn splicing_requires_its_own_permission() {
		let directory = test_directory("splice-permission");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let manager = store
			.create_key("manager", vec![CHANNELS_MANAGE_PERMISSION.to_string()], &admin)
			.unwrap()
			.info;
		let splicer = store
			.create_key("splicer", vec![CHANNELS_SPLICE_PERMISSION.to_string()], &admin)
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
			.create_key("delegated-splicer", vec![CHANNELS_SPLICE_PERMISSION.to_string()], &manager,)
			.is_err());
		let reloaded = ApiKeyStore::load_or_create(&directory).unwrap();
		assert!(reloaded.list_keys().unwrap().contains(&splicer));
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn concurrent_revocations_preserve_the_final_admin() {
		let directory = test_directory("concurrent-revoke");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let first = store.list_keys().unwrap().remove(0);
		let second = store
			.create_key("second-admin", vec![ADMIN_PERMISSION.to_string()], &first)
			.unwrap()
			.info;
		let barrier = std::sync::Barrier::new(2);
		std::thread::scope(|scope| {
			let handles: Vec<_> = [&first, &second]
				.into_iter()
				.map(|admin| {
					let store = &store;
					let barrier = &barrier;
					scope.spawn(move || {
						barrier.wait();
						store.revoke_key(&admin.id, admin)
					})
				})
				.collect();
			let results: Vec<_> =
				handles.into_iter().map(|handle| handle.join().unwrap()).collect();
			assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
			let error = results.into_iter().find_map(Result::err).unwrap();
			assert_eq!(error.error_code, LdkServerErrorCode::InvalidRequestError);
		});
		let keys = store.list_keys().unwrap();
		assert_eq!(keys.len(), 1);
		assert!(keys[0].is_admin());
		assert_eq!(ApiKeyStore::load_or_create(&directory).unwrap().list_keys().unwrap(), keys);
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn refuses_to_revoke_final_admin() {
		let directory = test_directory("final-admin");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);

		let error = store.revoke_key(&admin.id, &admin).unwrap_err();
		assert_eq!(error.error_code, LdkServerErrorCode::InvalidRequestError);
		assert!(store.keys.read().unwrap().contains_key(&admin.id));

		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn authentication_binds_rpc_method() {
		let directory = test_directory("method-binding");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let keys = store.keys.read().unwrap();
		let record = keys.values().next().unwrap();
		let body = b"framed protobuf body";
		let timestamp = now();
		let hmac =
			compute_auth_hmac(&record.secret, &record.info.id, GET_NODE_INFO_PATH, timestamp, body);
		let header = format!("HMAC {}:{}:{}", record.info.id, timestamp, hmac);

		assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&header), body).is_ok());
		assert_eq!(
			store.authenticate(GET_BALANCES_PATH, Some(&header), body).unwrap_err().error_code,
			LdkServerErrorCode::AuthError
		);

		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn authentication_rejects_missing_and_malformed_headers() {
		let directory = test_directory("malformed-auth");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let keys = store.keys.read().unwrap();
		let record = keys.values().next().unwrap();
		let id = &record.info.id;
		let timestamp = now();
		let body = b"framed protobuf body";
		let signature = compute_auth_hmac(&record.secret, id, GET_NODE_INFO_PATH, timestamp, body);
		let valid_header = format!("HMAC {id}:{timestamp}:{signature}");
		assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&valid_header), body).is_ok());

		let headers = [
			None,
			Some(String::new()),
			Some(format!("{id}:{timestamp}:{signature}")),
			Some(format!("Bearer {id}:{timestamp}:{signature}")),
			Some(format!("HMAC {timestamp}:{signature}")),
			Some(format!("HMAC {id}:{timestamp}")),
			Some(format!("{valid_header}:extra")),
			Some(format!("HMAC :{timestamp}:{signature}")),
			Some(format!("HMAC invalid-id:{timestamp}:{signature}")),
			Some(format!("HMAC {id}:not-a-timestamp:{signature}")),
			Some(format!("HMAC {id}:18446744073709551616:{signature}")),
			Some(format!("HMAC {id}:{timestamp}:")),
			Some(format!("HMAC {id}:{timestamp}:deadbeef")),
			Some(format!("HMAC {id}:{timestamp}:{}", "z".repeat(64))),
		];
		for (index, header) in headers.iter().enumerate() {
			let error =
				store.authenticate(GET_NODE_INFO_PATH, header.as_deref(), body).unwrap_err();
			assert_eq!(error.error_code, LdkServerErrorCode::AuthError, "header case {index}");
		}
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn authentication_rejects_expired_and_future_timestamps() {
		let directory = test_directory("expired-auth");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let keys = store.keys.read().unwrap();
		let record = keys.values().next().unwrap();
		let id = &record.info.id;
		let body = b"framed protobuf body";
		let current_time = now();
		for timestamp in [current_time - 600, current_time + 600] {
			let signature =
				compute_auth_hmac(&record.secret, id, GET_NODE_INFO_PATH, timestamp, body);
			let header = format!("HMAC {id}:{timestamp}:{signature}");
			let error = store.authenticate(GET_NODE_INFO_PATH, Some(&header), body).unwrap_err();
			assert_eq!(error.error_code, LdkServerErrorCode::AuthError);
			assert_eq!(error.message, "Request timestamp expired");
		}
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn authentication_rejects_wrong_secrets_and_modified_bodies() {
		let directory = test_directory("modified-auth");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let keys = store.keys.read().unwrap();
		let record = keys.values().next().unwrap();
		let id = &record.info.id;
		let body = b"signed request body";
		let timestamp = now();
		for (secret, request_body) in [
			("wrong secret", body.as_slice()),
			(record.secret.as_str(), b"modified request body".as_slice()),
		] {
			let signature = compute_auth_hmac(secret, id, GET_NODE_INFO_PATH, timestamp, body);
			let header = format!("HMAC {id}:{timestamp}:{signature}");
			let error =
				store.authenticate(GET_NODE_INFO_PATH, Some(&header), request_body).unwrap_err();
			assert_eq!(error.error_code, LdkServerErrorCode::AuthError);
			assert_eq!(error.message, "Invalid credentials");
		}
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn authentication_rejects_unknown_and_revoked_keys() {
		let directory = test_directory("revoked-auth");
		let store = ApiKeyStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let reader =
			store.create_key("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();
		let body = b"framed protobuf body";
		let timestamp = now();
		let signature =
			compute_auth_hmac(&reader.secret, &reader.info.id, GET_NODE_INFO_PATH, timestamp, body);
		let header = format!("HMAC {}:{timestamp}:{signature}", reader.info.id);
		assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&header), body).is_ok());
		store.revoke_key(&reader.info.id, &admin).unwrap();
		let unknown_id = compute_key_id("unknown secret");
		let unknown_signature =
			compute_auth_hmac("unknown secret", &unknown_id, GET_NODE_INFO_PATH, timestamp, body);
		for header in [header, format!("HMAC {unknown_id}:{timestamp}:{unknown_signature}")] {
			let error = store.authenticate(GET_NODE_INFO_PATH, Some(&header), body).unwrap_err();
			assert_eq!(error.error_code, LdkServerErrorCode::AuthError);
			assert_eq!(error.message, "Invalid credentials");
		}
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn rejects_unknown_and_mixed_admin_permissions() {
		assert!(validate_permissions(vec!["unknown:permission".to_string()]).is_err());
		assert!(validate_permissions(vec![
			ADMIN_PERMISSION.to_string(),
			NODE_READ_PERMISSION.to_string(),
		])
		.is_err());
		assert!(matches!(
			method_authorization(GET_PERMISSIONS_PATH),
			MethodAuthorization::AuthenticatedOnly
		));
		assert!(matches!(
			method_authorization("FutureUnclassifiedRpc"),
			MethodAuthorization::Unknown
		));
	}

	fn now() -> u64 {
		std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
	}

	fn test_directory(name: &str) -> PathBuf {
		let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
		let directory = std::env::temp_dir()
			.join(format!("ldk-server-api-key-test-{name}-{}-{count}", std::process::id()));
		let _ = fs::remove_dir_all(&directory);
		fs::create_dir(&directory).unwrap();
		directory
	}
}
