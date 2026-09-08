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

use hex::{DisplayHex, FromHex};
use ldk_node::bitcoin::hashes::{sha256, Hash};
use ldk_server_grpc::endpoints::{
	BOLT11_CLAIM_FOR_ID_PATH, BOLT11_FAIL_FOR_ID_PATH, BOLT11_RECEIVE_FOR_HASH_PATH,
	BOLT11_RECEIVE_PATH, BOLT11_RECEIVE_VARIABLE_AMOUNT_VIA_JIT_CHANNEL_PATH,
	BOLT11_RECEIVE_VIA_JIT_CHANNEL_PATH, BOLT11_SEND_PATH, BOLT11_SEND_UNDERPAYING_PATH,
	BOLT12_CREATE_PAYER_PROOF_PATH, BOLT12_RECEIVE_PATH, BOLT12_RECEIVE_REFUND_PATH,
	BOLT12_SEND_PATH, BOLT12_SEND_REFUND_PATH, CLOSE_CHANNEL_PATH, CONNECT_PEER_PATH,
	CREATE_MACAROON_PATH, DECODE_INVOICE_PATH, DECODE_OFFER_PATH, DISCONNECT_PEER_PATH,
	EXPORT_PATHFINDING_SCORES_PATH, FORCE_CLOSE_CHANNEL_PATH, GET_BALANCES_PATH,
	GET_NODE_INFO_PATH, GET_PAYMENT_DETAILS_PATH, GET_PERMISSIONS_PATH, GRAPH_GET_CHANNEL_PATH,
	GRAPH_GET_NODE_PATH, GRAPH_LIST_CHANNELS_PATH, GRAPH_LIST_NODES_PATH, LIST_CHANNELS_PATH,
	LIST_FORWARDED_PAYMENTS_PATH, LIST_MACAROONS_PATH, LIST_PAYMENTS_PATH, LIST_PEERS_PATH,
	ONCHAIN_RECEIVE_PATH, ONCHAIN_SEND_PATH, OPEN_CHANNEL_PATH, REVOKE_MACAROON_PATH,
	SIGN_MESSAGE_PATH, SPLICE_IN_PATH, SPLICE_OUT_PATH, SPONTANEOUS_SEND_PATH,
	SUBSCRIBE_EVENTS_PATH, UNIFIED_SEND_PATH, UPDATE_CHANNEL_CONFIG_PATH, VERIFY_SIGNATURE_PATH,
};
use ldk_server_grpc::macaroon::{Macaroon, MAX_MACAROON_BYTES};
use ldk_server_grpc::permissions::{
	ADMIN_PERMISSION, ALL_PERMISSIONS, CHANNELS_FORCE_CLOSE_PERMISSION, CHANNELS_MANAGE_PERMISSION,
	CHANNELS_READ_PERMISSION, CHANNELS_SPLICE_PERMISSION, EVENTS_READ_PERMISSION,
	GRAPH_READ_PERMISSION, INVOICES_CREATE_PERMISSION, MACAROONS_MANAGE_PERMISSION,
	MESSAGES_SIGN_PERMISSION, MESSAGES_VERIFY_PERMISSION, NODE_READ_PERMISSION,
	ONCHAIN_RECEIVE_PERMISSION, ONCHAIN_SEND_PERMISSION, PAYMENTS_CLAIM_PERMISSION,
	PAYMENTS_READ_PERMISSION, PAYMENTS_SEND_PERMISSION, PEERS_MANAGE_PERMISSION,
	PEERS_READ_PERMISSION, UTILITIES_READ_PERMISSION,
};
use ring::hmac;
use serde::Deserialize;

use crate::api::error::{LdkServerError, LdkServerErrorCode};
use crate::util::{create_dir_all_private, read_to_string_with_limit, write_new};

const MACAROON_FILE_SIZE_LIMIT: usize = 16384;
const MACAROONS_DIR: &str = "macaroons";
const ADMIN_KEY_FILE: &str = "admin.toml";
const ADMIN_MACAROON_FILE: &str = "admin.macaroon";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MacaroonInfo {
	pub(crate) id: String,
	pub(crate) name: String,
	pub(crate) permissions: BTreeSet<String>,
	pub(crate) caveats: Vec<String>,
}

impl MacaroonInfo {
	pub(crate) fn is_admin(&self) -> bool {
		self.permissions.contains(ADMIN_PERMISSION)
	}

	pub(crate) fn allows(&self, permission: &str) -> bool {
		self.is_admin() || self.permissions.contains(permission)
	}
}

#[derive(Debug)]
pub(crate) struct CreatedMacaroon {
	pub(crate) info: MacaroonInfo,
	pub(crate) token: String,
}

#[derive(Debug)]
struct MacaroonRecord {
	info: Arc<MacaroonInfo>,
	secret: String,
	path: PathBuf,
}

#[derive(Deserialize)]
struct StoredMacaroon {
	id: String,
	name: String,
	key: String,
	permissions: Vec<String>,
	#[serde(default)]
	caveats: Vec<String>,
}

pub(crate) struct MacaroonStore {
	keys: RwLock<HashMap<String, Arc<MacaroonRecord>>>,
	management: Mutex<()>,
	directory: PathBuf,
}

impl MacaroonStore {
	pub(crate) fn load_or_create(storage_dir: &Path) -> io::Result<Self> {
		let macaroon_dir = storage_dir.join(MACAROONS_DIR);
		create_dir_all_private(&macaroon_dir)?;
		fs::set_permissions(&macaroon_dir, fs::Permissions::from_mode(0o700))?;
		let directory = macaroon_dir.join("roots");
		create_dir_all_private(&directory)?;
		fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;

		let mut store =
			Self { keys: RwLock::new(HashMap::new()), management: Mutex::new(()), directory };
		store.load_key_files()?;
		if store
			.keys
			.get_mut()
			.map_err(|_| io::Error::other("macaroon store lock is poisoned"))?
			.is_empty()
		{
			store.create_initial_admin()?;
		}
		let admin_path = macaroon_dir.join(ADMIN_MACAROON_FILE);
		if !admin_path.try_exists()? {
			let keys =
				store.keys.get_mut().map_err(|_| invalid_data("Macaroon store lock poisoned"))?;
			if let Some(admin) = keys
				.values()
				.find(|record| record.path.file_name().is_some_and(|name| name == ADMIN_KEY_FILE))
			{
				let token = mint_token(&admin.info, &admin.secret).map_err(invalid_data)?;
				write_private_file(&admin_path, token.as_bytes())?;
			}
		}
		Ok(store)
	}

	fn load_key_files(&mut self) -> io::Result<()> {
		let keys =
			self.keys.get_mut().map_err(|_| io::Error::other("macaroon store lock is poisoned"))?;
		for entry in fs::read_dir(&self.directory)? {
			let path = entry?.path();
			if path.extension().is_none_or(|extension| extension != "toml") {
				continue;
			}

			let contents = read_to_string_with_limit(&path, MACAROON_FILE_SIZE_LIMIT)?;
			let stored: StoredMacaroon = toml::from_str(&contents).map_err(|error| {
				invalid_data(format!("Failed to parse macaroon file {}: {error}", path.display()))
			})?;
			let record = record_from_stored(stored, path)?;
			if keys.values().any(|existing| existing.info.name == record.info.name) {
				return Err(invalid_data(format!("Duplicate macaroon name: {}", record.info.name)));
			}
			if keys.insert(record.info.id.clone(), Arc::new(record)).is_some() {
				return Err(invalid_data("Duplicate macaroon ID"));
			}
		}
		Ok(())
	}

	fn create_initial_admin(&mut self) -> io::Result<()> {
		let secret = generate_secret()?;
		let info = MacaroonInfo {
			id: compute_key_id(&secret),
			name: "admin".to_string(),
			permissions: BTreeSet::from([ADMIN_PERMISSION.to_string()]),
			caveats: Vec::new(),
		};
		let path = self.directory.join(ADMIN_KEY_FILE);
		write_key_file(&path, &info, &secret)?;
		self.keys
			.get_mut()
			.map_err(|_| io::Error::other("macaroon store lock is poisoned"))?
			.insert(
				info.id.clone(),
				Arc::new(MacaroonRecord { info: Arc::new(info), secret, path }),
			);

		Ok(())
	}

	pub(crate) fn authenticate(
		&self, method: &str, auth_header: Option<&str>,
	) -> Result<Arc<MacaroonInfo>, LdkServerError> {
		let invalid =
			|| LdkServerError::new(LdkServerErrorCode::AuthError, "Invalid macaroon credentials");
		let token = auth_header.ok_or_else(invalid)?;
		if token.len() > MAX_MACAROON_BYTES * 2 {
			return Err(invalid());
		}
		let bytes = Vec::<u8>::from_hex(token).map_err(|_| invalid())?;
		let macaroon = Macaroon::deserialize(&bytes).map_err(|_| invalid())?;
		let id = std::str::from_utf8(macaroon.identifier()).map_err(|_| invalid())?;
		let record = self
			.keys
			.read()
			.map_err(|_| key_store_lock_error())?
			.get(id)
			.cloned()
			.ok_or_else(invalid)?;
		let root = Vec::<u8>::from_hex(&record.secret).map_err(|_| invalid())?;
		if !macaroon.verify_signature(&root, sign, |key, data, signature| {
			hmac::verify(&hmac::Key::new(hmac::HMAC_SHA256, key), data, signature).is_ok()
		}) {
			return Err(invalid());
		}
		let mut info = (*record.info).clone();
		info.caveats = macaroon
			.caveats()
			.iter()
			.map(|caveat| {
				String::from_utf8(caveat.clone())
					.map_err(|_| authorization_error("Invalid macaroon caveat"))
			})
			.collect::<Result<_, _>>()?;
		// Stored limits remain an upper bound, including for tokens minted through the API.
		for caveat in record.info.caveats.iter().chain(info.caveats.iter()) {
			check_caveat(caveat, method, &mut info.permissions)?;
		}
		Ok(Arc::new(info))
	}

	// Call management operations from a blocking thread. Authentication never takes this mutex.
	pub(crate) fn create_key(
		&self, name: &str, permissions: Vec<String>, issuer: &MacaroonInfo,
	) -> Result<CreatedMacaroon, LdkServerError> {
		self.create_key_with_writer(name, permissions, issuer, write_key_file)
	}

	fn create_key_with_writer(
		&self, name: &str, permissions: Vec<String>, issuer: &MacaroonInfo,
		write: impl FnOnce(&Path, &MacaroonInfo, &str) -> io::Result<()>,
	) -> Result<CreatedMacaroon, LdkServerError> {
		validate_name(name)?;
		let permissions = validate_permissions(permissions).map_err(invalid_request)?;
		let _management = self.management.lock().map_err(|_| key_store_lock_error())?;
		if !issuer.allows(MACAROONS_MANAGE_PERMISSION) {
			return Err(authorization_error("Macaroon management permission required"));
		}
		let mut issuer_permissions = issuer.permissions.clone();
		for caveat in &issuer.caveats {
			check_caveat(caveat, CREATE_MACAROON_PATH, &mut issuer_permissions)?;
		}
		if !issuer_permissions.contains(ADMIN_PERMISSION)
			&& !issuer_permissions.contains(MACAROONS_MANAGE_PERMISSION)
		{
			return Err(authorization_error("Macaroon management permission required"));
		}
		let secret = generate_secret().map_err(internal_error)?;
		let info = MacaroonInfo {
			id: compute_key_id(&secret),
			name: name.to_string(),
			permissions,
			caveats: issuer.caveats.clone(),
		};
		let token = mint_token(&info, &secret).map_err(invalid_request)?;
		{
			let keys = self.keys.read().map_err(|_| key_store_lock_error())?;
			if !keys.contains_key(&issuer.id) {
				return Err(LdkServerError::new(
					LdkServerErrorCode::AuthError,
					"Invalid credentials",
				));
			}
			if keys.values().any(|record| record.info.name == name) {
				return Err(invalid_request(format!("macaroon name already exists: {name}")));
			}
			if !issuer_permissions.contains(ADMIN_PERMISSION)
				&& info
					.permissions
					.iter()
					.any(|permission| !issuer_permissions.contains(permission))
			{
				return Err(authorization_error(
					"Cannot grant a permission that the calling key does not have",
				));
			}
			if keys.contains_key(&info.id) {
				return Err(internal_error("Generated a duplicate macaroon ID"));
			}
		}
		let path = self.directory.join(format!("{}.toml", info.id));
		write(&path, &info, &secret).map_err(internal_error)?;
		let record =
			Arc::new(MacaroonRecord { info: Arc::new(info.clone()), secret: secret.clone(), path });
		self.keys.write().map_err(|_| key_store_lock_error())?.insert(info.id.clone(), record);
		Ok(CreatedMacaroon { info, token })
	}

	pub(crate) fn list_keys(&self) -> Result<Vec<MacaroonInfo>, LdkServerError> {
		let records = self.keys.read().map_err(|_| key_store_lock_error())?;
		let mut keys: Vec<_> = records.values().map(|record| (*record.info).clone()).collect();
		keys.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
		Ok(keys)
	}

	pub(crate) fn revoke_key(&self, id: &str, issuer: &MacaroonInfo) -> Result<(), LdkServerError> {
		if !is_hex(id, 32) {
			return Err(invalid_request(
				"macaroon ID must contain exactly 32 hexadecimal characters",
			));
		}
		let _management = self.management.lock().map_err(|_| key_store_lock_error())?;
		if !issuer.allows(MACAROONS_MANAGE_PERMISSION) {
			return Err(authorization_error("Macaroon management permission required"));
		}
		let mut issuer_permissions = issuer.permissions.clone();
		for caveat in &issuer.caveats {
			check_caveat(caveat, REVOKE_MACAROON_PATH, &mut issuer_permissions)?;
		}
		if !issuer_permissions.contains(ADMIN_PERMISSION)
			&& !issuer_permissions.contains(MACAROONS_MANAGE_PERMISSION)
		{
			return Err(authorization_error("Macaroon management permission required"));
		}
		let keys = self.keys.read().map_err(|_| key_store_lock_error())?;
		if !keys.contains_key(&issuer.id) {
			return Err(LdkServerError::new(LdkServerErrorCode::AuthError, "Invalid credentials"));
		}
		let record =
			keys.get(id).ok_or_else(|| invalid_request(format!("Unknown macaroon ID: {id}")))?;
		if !issuer_permissions.contains(ADMIN_PERMISSION)
			&& (record.info.is_admin()
				|| record
					.info
					.permissions
					.iter()
					.any(|permission| !issuer_permissions.contains(permission)))
		{
			return Err(authorization_error(
				"Cannot revoke a key with permissions that the calling key does not have",
			));
		}
		if is_unrestricted_admin(&record.info)
			&& keys.values().filter(|record| is_unrestricted_admin(&record.info)).count() == 1
		{
			return Err(invalid_request("Cannot revoke the final admin macaroon"));
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
		CREATE_MACAROON_PATH | LIST_MACAROONS_PATH | REVOKE_MACAROON_PATH => {
			MethodAuthorization::Permission(MACAROONS_MANAGE_PERMISSION)
		},
		GET_PERMISSIONS_PATH => MethodAuthorization::AuthenticatedOnly,
		_ => MethodAuthorization::Unknown,
	}
}

fn sign(key: &[u8], data: &[u8]) -> [u8; 32] {
	hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, key), data).as_ref().try_into().unwrap()
}

fn mint_token(info: &MacaroonInfo, secret: &str) -> Result<String, &'static str> {
	let root = Vec::<u8>::from_hex(secret).map_err(|_| "Invalid macaroon root key")?;
	let mut macaroon = Macaroon::mint(&root, info.id.as_bytes(), sign)?;
	let permissions = info.permissions.iter().cloned().collect::<Vec<_>>().join(",");
	macaroon.attenuate(format!("permissions = {permissions}").as_bytes(), sign)?;
	for caveat in &info.caveats {
		macaroon.attenuate(caveat.as_bytes(), sign)?;
	}
	Ok(macaroon.serialize().to_lower_hex_string())
}

fn check_caveat(
	caveat: &str, method: &str, permissions: &mut BTreeSet<String>,
) -> Result<(), LdkServerError> {
	if let Some(value) = caveat.strip_prefix("permissions = ") {
		let allowed = validate_permissions(value.split(',').map(str::to_string).collect())
			.map_err(authorization_error)?;
		if permissions.contains(ADMIN_PERMISSION) {
			*permissions = allowed;
		} else if !allowed.contains(ADMIN_PERMISSION) {
			permissions.retain(|p| allowed.contains(p));
		}
	} else if let Some(value) = caveat.strip_prefix("time-before = ") {
		let expiry =
			value.parse::<u64>().map_err(|_| authorization_error("Invalid expiry caveat"))?;
		if value != expiry.to_string() {
			return Err(authorization_error("Invalid expiry caveat"));
		}
		let now = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map_err(internal_error)?
			.as_secs();
		if now >= expiry {
			return Err(authorization_error("Macaroon expired"));
		}
	} else if let Some(value) = caveat.strip_prefix("method = ") {
		if value != method {
			return Err(authorization_error("Macaroon does not allow this RPC method"));
		}
	} else {
		return Err(authorization_error("Unknown macaroon caveat"));
	}
	Ok(())
}

fn is_unrestricted_admin(info: &MacaroonInfo) -> bool {
	info.is_admin() && info.caveats.iter().all(|c| c == "permissions = admin")
}

fn record_from_stored(stored: StoredMacaroon, path: PathBuf) -> io::Result<MacaroonRecord> {
	if !is_hex(&stored.key, 64) {
		return Err(invalid_data(format!("Invalid macaroon in {}", path.display())));
	}
	if !is_hex(&stored.id, 32) || stored.id != compute_key_id(&stored.key) {
		return Err(invalid_data(format!("Invalid macaroon ID in {}", path.display())));
	}
	validate_name_value(&stored.name).map_err(invalid_data)?;
	let permissions = validate_permissions(stored.permissions).map_err(invalid_data)?;
	if stored.caveats.len() >= ldk_server_grpc::macaroon::MAX_CAVEATS
		|| stored.caveats.iter().any(|c| !c.is_ascii() || c.bytes().any(|b| b < 32 || b == 127))
	{
		return Err(invalid_data("Invalid stored macaroon caveats"));
	}
	Ok(MacaroonRecord {
		info: Arc::new(MacaroonInfo {
			id: stored.id,
			name: stored.name,
			permissions,
			caveats: stored.caveats,
		}),
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
			"macaroon name must contain 1 to 64 ASCII letters, numbers, hyphens, or underscores"
				.to_string(),
		);
	}
	Ok(())
}

fn validate_permissions(permissions: Vec<String>) -> Result<BTreeSet<String>, String> {
	let permissions: BTreeSet<_> = permissions.into_iter().collect();
	if permissions.is_empty() {
		return Err("At least one macaroon permission is required".to_string());
	}
	for permission in &permissions {
		if !ALL_PERMISSIONS.contains(&permission.as_str()) {
			return Err(format!("Unknown macaroon permission: {permission}"));
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

fn write_key_file(path: &Path, info: &MacaroonInfo, secret: &str) -> io::Result<()> {
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

fn write_private_file(path: &Path, contents: &[u8]) -> io::Result<()> {
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
	internal_error("macaroon store lock is poisoned")
}

fn internal_error(message: impl std::fmt::Display) -> LdkServerError {
	LdkServerError::new(LdkServerErrorCode::InternalServerError, message.to_string())
}

#[cfg(test)]
mod tests {
	use std::sync::atomic::{AtomicU32, Ordering};

	use ldk_server_grpc::endpoints::{GET_BALANCES_PATH, GET_NODE_INFO_PATH};
	use ldk_server_grpc::permissions::{MACAROONS_MANAGE_PERMISSION, NODE_READ_PERMISSION};

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
			("CreateMacaroon", Some("macaroons:manage")),
			("ListMacaroons", Some("macaroons:manage")),
			("RevokeMacaroon", Some("macaroons:manage")),
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
			let mut key = MacaroonInfo {
				id: "test".to_string(),
				name: "test".to_string(),
				permissions: BTreeSet::new(),
				caveats: Vec::new(),
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
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		let keys = store.list_keys().unwrap();

		assert_eq!(keys.len(), 1);
		assert_eq!(keys[0].name, "admin");
		assert!(keys[0].is_admin());
		let admin_path = directory.join(MACAROONS_DIR).join("roots").join(ADMIN_KEY_FILE);
		assert!(admin_path.exists());
		assert_eq!(fs::metadata(admin_path).unwrap().permissions().mode() & 0o777, 0o400);
		assert_eq!(
			fs::metadata(directory.join(MACAROONS_DIR)).unwrap().permissions().mode() & 0o777,
			0o700
		);

		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn load_macaroon_rejects_oversized_toml() {
		let directory = test_directory("oversized-key-toml");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		let path = directory.join(MACAROONS_DIR).join("roots").join("oversized.toml");
		fs::write(&path, vec![b' '; MACAROON_FILE_SIZE_LIMIT + 1]).unwrap();
		drop(store);
		let error = MacaroonStore::load_or_create(&directory).err().unwrap();
		assert_eq!(error.kind(), io::ErrorKind::InvalidData);
		assert!(error.to_string().contains("exceeds"));
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn load_rejects_duplicate_key_names_and_ids() {
		for duplicate in ["name", "ID"] {
			let directory = test_directory("duplicate-key");
			let store = MacaroonStore::load_or_create(&directory).unwrap();
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
			write_key_file(
				&directory.join(MACAROONS_DIR).join("roots").join("duplicate.toml"),
				&info,
				&secret,
			)
			.unwrap();
			drop(store);

			let error = MacaroonStore::load_or_create(&directory).err().unwrap();
			assert_eq!(error.kind(), io::ErrorKind::InvalidData);
			assert!(error.to_string().contains(&format!("Duplicate macaroon {duplicate}")));
			fs::remove_dir_all(directory).unwrap();
		}
	}

	#[test]
	fn creates_lists_revokes_and_reloads_key() {
		let directory = test_directory("lifecycle");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let created =
			store.create_key("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();

		assert_eq!(store.list_keys().unwrap().len(), 2);
		assert!(created.info.allows(NODE_READ_PERMISSION));
		assert!(!created.info.is_admin());
		drop(store);

		let reloaded = MacaroonStore::load_or_create(&directory).unwrap();
		assert_eq!(reloaded.list_keys().unwrap().len(), 2);
		reloaded.revoke_key(&created.info.id, &admin).unwrap();
		assert_eq!(reloaded.list_keys().unwrap(), vec![admin]);

		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn revokes_key_when_its_file_is_missing() {
		let directory = test_directory("revoke-missing-file");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let reader =
			store.create_key("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();
		let header = reader.token.clone();
		fs::remove_file(
			directory.join(MACAROONS_DIR).join("roots").join(format!("{}.toml", reader.info.id)),
		)
		.unwrap();
		assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&header)).is_ok());

		store.revoke_key(&reader.info.id, &admin).unwrap();
		assert_eq!(
			store.authenticate(GET_NODE_INFO_PATH, Some(&header)).unwrap_err().error_code,
			LdkServerErrorCode::AuthError
		);
		assert_eq!(store.list_keys().unwrap(), vec![admin.clone()]);
		assert_eq!(
			MacaroonStore::load_or_create(&directory).unwrap().list_keys().unwrap(),
			vec![admin]
		);
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn revoke_rejects_malformed_ids_without_echoing_them() {
		let directory = test_directory("revoke-invalid-id");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		for id in [String::new(), "a".repeat(31), "a".repeat(33), "z".repeat(32), "a".repeat(8192)]
		{
			let error = store.revoke_key(&id, &admin).unwrap_err();
			assert_eq!(error.error_code, LdkServerErrorCode::InvalidRequestError);
			assert_eq!(error.message, "macaroon ID must contain exactly 32 hexadecimal characters");
		}
		assert_eq!(store.list_keys().unwrap(), vec![admin]);
		fs::remove_dir_all(directory).unwrap();
	}

	#[tokio::test]
	async fn authentication_continues_during_key_file_write() {
		use std::time::Duration;
		let directory = test_directory("slow-key-write");
		let store = Arc::new(MacaroonStore::load_or_create(&directory).unwrap());
		let admin = store.list_keys().unwrap().remove(0);
		let reader =
			store.create_key("reader", vec![NODE_READ_PERMISSION.to_string()], &admin).unwrap();
		let header = reader.token.clone();
		let first = store.authenticate(GET_NODE_INFO_PATH, Some(&header)).unwrap();
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
			let key = auth_store.authenticate(GET_NODE_INFO_PATH, Some(&header)).unwrap();
			assert!(!auth_store.list_keys().unwrap().iter().any(|key| key.name == "pending"));
			key
		});
		let result = tokio::time::timeout(Duration::from_secs(1), auth).await;
		// Release the writer even if authentication timed out, so the test cannot hang.
		release_tx.send(()).unwrap();
		writer.await.unwrap().unwrap();
		let second = result.expect("Authentication waited for disk I/O").unwrap();
		assert_eq!(first, second);
		assert!(store.list_keys().unwrap().iter().any(|key| key.name == "pending"));
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn failed_key_write_does_not_publish_key() {
		let directory = test_directory("failed-key-write");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
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
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let manager = store
			.create_key(
				"manager",
				vec![MACAROONS_MANAGE_PERMISSION.to_string(), NODE_READ_PERMISSION.to_string()],
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
		let store = MacaroonStore::load_or_create(&directory).unwrap();
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
		let reloaded = MacaroonStore::load_or_create(&directory).unwrap();
		assert_eq!(store.list_keys().unwrap(), reloaded.list_keys().unwrap());
		assert_eq!(reloaded.list_keys().unwrap().len(), 2);
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn scoped_manager_cannot_escalate_or_revoke_admin() {
		let directory = test_directory("delegation");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let manager = store
			.create_key(
				"manager",
				vec![MACAROONS_MANAGE_PERMISSION.to_string(), NODE_READ_PERMISSION.to_string()],
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
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let manager = store
			.create_key(
				"manager",
				vec![MACAROONS_MANAGE_PERMISSION.to_string(), NODE_READ_PERMISSION.to_string()],
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
		assert_eq!(MacaroonStore::load_or_create(&directory).unwrap().list_keys().unwrap(), keys);
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn splicing_requires_its_own_permission() {
		let directory = test_directory("splice-permission");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
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
		let reloaded = MacaroonStore::load_or_create(&directory).unwrap();
		assert!(reloaded.list_keys().unwrap().contains(&splicer));
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn concurrent_revocations_preserve_the_final_admin() {
		let directory = test_directory("concurrent-revoke");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
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
		assert_eq!(MacaroonStore::load_or_create(&directory).unwrap().list_keys().unwrap(), keys);
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn refuses_to_revoke_final_admin() {
		let directory = test_directory("final-admin");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);

		let error = store.revoke_key(&admin.id, &admin).unwrap_err();
		assert_eq!(error.error_code, LdkServerErrorCode::InvalidRequestError);
		assert!(store.keys.read().unwrap().contains_key(&admin.id));

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

	fn restrict(token: &str, caveats: &[&str]) -> String {
		let bytes = Vec::<u8>::from_hex(token).unwrap();
		let mut m = Macaroon::deserialize(&bytes).unwrap();
		for caveat in caveats {
			m.attenuate(caveat.as_bytes(), sign).unwrap();
		}
		m.serialize().to_lower_hex_string()
	}

	fn admin_token(store: &MacaroonStore) -> String {
		let keys = store.keys.read().unwrap();
		let record = keys.values().find(|r| r.info.name == "admin").unwrap();
		mint_token(&record.info, &record.secret).unwrap()
	}

	#[test]
	fn standard_signatures_match_reference_implementation() {
		for (index, line) in include_str!("../../ldk-server-grpc/tests/data/macaroons-v2.txt")
			.lines()
			.filter(|line| !line.starts_with('#'))
			.enumerate()
		{
			let fields: Vec<_> = line.split_whitespace().collect();
			let root = Vec::<u8>::from_hex(fields[0]).unwrap();
			let id = Vec::<u8>::from_hex(fields[1]).unwrap();
			let mut bytes = Vec::<u8>::from_hex(fields[2]).unwrap();
			let m = Macaroon::deserialize(&bytes).unwrap();
			let verify = |key: &[u8], data: &[u8], sig: &[u8; 32]| {
				hmac::verify(&hmac::Key::new(hmac::HMAC_SHA256, key), data, sig).is_ok()
			};
			assert!(m.verify_signature(&root, sign, verify));
			assert!(!m.verify_signature(b"incorrect root", sign, verify));
			let mut issued = Macaroon::mint(&root, &id, sign).unwrap();
			for caveat in m.caveats() {
				issued.attenuate(caveat, sign).unwrap();
			}
			if index != 3 {
				// The reference writes an empty location; our issuer omits this optional hint.
				let mut without_empty_location = bytes.clone();
				assert_eq!(&without_empty_location[1..3], &[1, 0]);
				without_empty_location.drain(1..3);
				assert_eq!(issued.serialize(), without_empty_location);
			} // Case 3 has a location hint.
			*bytes.last_mut().unwrap() ^= 1;
			assert!(!Macaroon::deserialize(&bytes).unwrap().verify_signature(&root, sign, verify));
		}
	}

	#[test]
	fn attenuation_intersects_permissions_and_enforces_all_conditions() {
		let directory = test_directory("attenuation");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
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
		assert!(store.create_key("escalated", vec![ADMIN_PERMISSION.into()], &reader).is_err());
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
		assert!(store
			.authenticate(GET_NODE_INFO_PATH, Some(&restrict(&token, &[&future])))
			.is_ok());
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
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn rejects_forgery_and_revokes_all_attenuated_copies() {
		let directory = test_directory("revocation");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		let admin = store.list_keys().unwrap().remove(0);
		let reader = store.create_key("reader", vec![NODE_READ_PERMISSION.into()], &admin).unwrap();
		let token = restrict(&reader.token, &["method = GetNodeInfo"]);
		assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&token)).is_ok());
		// Change a caveat without recomputing the chain.
		let bytes = Vec::<u8>::from_hex(&token).unwrap();
		let mut modified = bytes.clone();
		let offset = modified.windows(11).position(|w| w == b"GetNodeInfo").unwrap();
		modified[offset] = b'X';
		assert!(store
			.authenticate(GET_NODE_INFO_PATH, Some(&modified.to_lower_hex_string()))
			.is_err());
		// Remove the last caveat while retaining the final signature.
		let mut removed = Vec::<u8>::from_hex(&reader.token).unwrap();
		let len = removed.len();
		removed[len - 32..].copy_from_slice(&bytes[bytes.len() - 32..]);
		assert!(store
			.authenticate(GET_NODE_INFO_PATH, Some(&removed.to_lower_hex_string()))
			.is_err());
		for header in [None, Some(""), Some("HMAC old-auth"), Some("deadbeef")] {
			assert_eq!(
				store.authenticate(GET_NODE_INFO_PATH, header).unwrap_err().error_code,
				LdkServerErrorCode::AuthError
			);
		}
		let root = { store.keys.read().unwrap().get(&reader.info.id).unwrap().secret.clone() };
		assert_ne!(reader.token, root);
		store.revoke_key(&reader.info.id, &admin).unwrap();
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
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn issued_children_inherit_expiry_and_method_restrictions() {
		let directory = test_directory("inherited-caveats");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		let expiry = format!("time-before = {}", now() + 3600);
		let token = restrict(&admin_token(&store), &[&expiry, "method = CreateMacaroon"]);
		let issuer = store.authenticate(CREATE_MACAROON_PATH, Some(&token)).unwrap();
		let created =
			store.create_key("child", vec![NODE_READ_PERMISSION.into()], &issuer).unwrap();
		assert!(created.info.caveats.contains(&expiry));
		assert!(created.info.caveats.contains(&"method = CreateMacaroon".to_string()));
		assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&created.token)).is_err());
		let reloaded = MacaroonStore::load_or_create(&directory).unwrap();
		assert!(reloaded.authenticate(GET_NODE_INFO_PATH, Some(&created.token)).is_err());
		let mut stale = (*issuer).clone();
		stale.caveats.push("time-before = 0".into());
		assert!(store.create_key("expired", vec![NODE_READ_PERMISSION.into()], &stale).is_err());
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn attenuation_limits_leave_token_unchanged() {
		let mut token = Macaroon::mint(b"key", b"id", sign).unwrap();
		for _ in 0..ldk_server_grpc::macaroon::MAX_CAVEATS {
			token.attenuate(b"permissions = admin", sign).unwrap();
		}
		let original = token.serialize();
		assert!(token.attenuate(b"permissions = admin", sign).is_err());
		assert_eq!(token.serialize(), original);
		let mut token = Macaroon::mint(b"key", b"id", sign).unwrap();
		let original = token.serialize();
		assert!(token.attenuate(&vec![b'x'; MAX_MACAROON_BYTES], sign).is_err());
		assert_eq!(token.serialize(), original);
	}

	#[test]
	fn bootstrap_token_is_private_and_recovers_with_the_same_root() {
		let directory = test_directory("bootstrap-token");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		let path = directory.join(MACAROONS_DIR).join(ADMIN_MACAROON_FILE);
		let original = fs::read_to_string(&path).unwrap();
		assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o400);
		assert!(store.authenticate(GET_NODE_INFO_PATH, Some(&original)).unwrap().is_admin());
		let roots = store.list_keys().unwrap();
		fs::remove_file(&path).unwrap();
		drop(store);
		let reloaded = MacaroonStore::load_or_create(&directory).unwrap();
		assert_eq!(reloaded.list_keys().unwrap(), roots);
		assert_eq!(fs::read_to_string(path).unwrap(), original);
		fs::remove_dir_all(directory).unwrap();
	}

	#[test]
	fn restricted_admin_does_not_replace_the_last_unrestricted_admin() {
		let directory = test_directory("restricted-admin");
		let store = MacaroonStore::load_or_create(&directory).unwrap();
		let token = restrict(&admin_token(&store), &["method = CreateMacaroon"]);
		let issuer = store.authenticate(CREATE_MACAROON_PATH, Some(&token)).unwrap();
		store.create_key("restricted-admin", vec![ADMIN_PERMISSION.into()], &issuer).unwrap();
		let admin = store.list_keys().unwrap().into_iter().find(|i| i.name == "admin").unwrap();
		assert!(store.revoke_key(&admin.id, &admin).is_err());
		fs::remove_dir_all(directory).unwrap();
	}

	fn now() -> u64 {
		std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
	}

	fn test_directory(name: &str) -> PathBuf {
		let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
		let directory = std::env::temp_dir()
			.join(format!("ldk-server-macaroon-test-{name}-{}-{count}", std::process::id()));
		let _ = fs::remove_dir_all(&directory);
		fs::create_dir(&directory).unwrap();
		directory
	}
}
