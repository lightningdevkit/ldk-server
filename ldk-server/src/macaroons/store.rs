// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Root lifecycle and request authentication.

use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use hex::FromHex;
use ldk_server_grpc::endpoints::{CREATE_MACAROON_PATH, REVOKE_MACAROON_PATH};
use ldk_server_grpc::permissions::ADMIN_PERMISSION;
use ldk_server_macaroons::{Macaroon, RequestBinding, MAX_MACAROON_BYTES};

use super::persistence::{
	compute_root_id, generate_secret, is_hex, record_from_stored, write_private_file,
	write_root_file, RootRecord, StoredRoot, ADMIN_MACAROON_FILE, ADMIN_ROOT_FILE, MACAROONS_DIR,
	MACAROON_FILE_SIZE_LIMIT,
};
use super::policy::{
	check_caveat, check_caveat_at, check_request_timestamp, is_unrestricted_admin,
	management_permissions, mint_token, unix_time, validate_name, validate_permissions,
};
use super::{
	auth_error, authorization_error, internal_error, invalid_data, invalid_request,
	store_lock_error, MacaroonInfo,
};
use crate::api::error::LdkServerError;
use crate::util::{create_dir_all_private, read_to_string_with_limit};

#[derive(Debug)]
pub(crate) struct CreatedMacaroon {
	pub(crate) info: MacaroonInfo,
	pub(crate) token: String,
}

/// The signature and header conditions passed, but the body has not been checked yet.
/// This must be completed with `finish_request` before executing the RPC.
#[derive(Debug)]
pub(crate) struct PendingMacaroonRequest {
	pub(crate) info: Arc<MacaroonInfo>,
	binding: RequestBinding,
}

pub(crate) struct MacaroonStore {
	roots: RwLock<HashMap<String, Arc<RootRecord>>>,
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
			Self { roots: RwLock::new(HashMap::new()), management: Mutex::new(()), directory };
		store.load_root_files()?;
		if store.roots_mut()?.is_empty() {
			store.create_initial_admin()?;
		}
		let admin_path = macaroon_dir.join(ADMIN_MACAROON_FILE);
		if !store.default_admin_token_is_valid(&admin_path)? {
			store.write_default_admin_token(&admin_path)?;
		}
		Ok(store)
	}

	fn roots_mut(&mut self) -> io::Result<&mut HashMap<String, Arc<RootRecord>>> {
		self.roots.get_mut().map_err(|_| io::Error::other("macaroon store lock is poisoned"))
	}

	fn default_admin_token_is_valid(&self, path: &Path) -> io::Result<bool> {
		if !path.try_exists()? {
			return Ok(false);
		}
		let token = match read_to_string_with_limit(path, MAX_MACAROON_BYTES * 2 + 2) {
			Ok(token) => token,
			Err(error) if error.kind() == io::ErrorKind::InvalidData => return Ok(false),
			Err(error) => {
				return Err(io::Error::new(
					error.kind(),
					format!("Failed to read default macaroon {}: {error}", path.display()),
				))
			},
		};
		// Check the signature only. Preserve valid restricted or expired credentials.
		let Ok((macaroon, _)) = self.verify_token(token.trim()) else {
			return Ok(false);
		};
		if macaroon.has_request_proof() {
			log::warn!(
				"Default macaroon file {} contains a request proof; restore a reusable credential",
				path.display()
			);
		}
		Ok(true)
	}

	fn write_default_admin_token(&mut self, path: &Path) -> io::Result<()> {
		let roots = self.roots_mut()?;
		if let Some(admin) = roots
			.values()
			.find(|record| record.path.file_name().is_some_and(|name| name == ADMIN_ROOT_FILE))
		{
			let token = mint_token(&admin.info, &admin.secret).map_err(invalid_data)?;
			write_private_file(path, token.as_bytes())?;
			log::info!("Wrote default macaroon: id={}", admin.info.id);
		} else {
			log::warn!(
				"Default admin.macaroon is missing or invalid and roots/admin.toml is absent; \
             use an existing admin credential to create a replacement and save its token \
             as admin.macaroon"
			);
		}
		Ok(())
	}

	fn load_root_files(&mut self) -> io::Result<()> {
		let entries = fs::read_dir(&self.directory)?;
		let roots = self.roots_mut()?;
		for entry in entries {
			let path = entry?.path();
			if path.extension().is_none_or(|extension| extension != "toml") {
				continue;
			}

			let contents = read_to_string_with_limit(&path, MACAROON_FILE_SIZE_LIMIT)?;
			let stored: StoredRoot = toml::from_str(&contents).map_err(|error| {
				invalid_data(format!("Failed to parse macaroon file {}: {error}", path.display()))
			})?;
			let record = record_from_stored(stored, path)?;
			if roots.values().any(|existing| existing.info.name == record.info.name) {
				return Err(invalid_data(format!("Duplicate macaroon name: {}", record.info.name)));
			}
			if roots.insert(record.info.id.clone(), Arc::new(record)).is_some() {
				return Err(invalid_data("Duplicate macaroon ID"));
			}
		}
		Ok(())
	}

	fn create_initial_admin(&mut self) -> io::Result<()> {
		let secret = generate_secret()?;
		let info = MacaroonInfo {
			id: compute_root_id(&secret),
			name: "admin".to_string(),
			permissions: BTreeSet::from([ADMIN_PERMISSION.to_string()]),
			caveats: Vec::new(),
		};
		let path = self.directory.join(ADMIN_ROOT_FILE);
		write_root_file(&path, &info, &secret)?;
		self.roots_mut()?
			.insert(info.id.clone(), Arc::new(RootRecord { info: Arc::new(info), secret, path }));

		Ok(())
	}

	fn verify_token(&self, token: &str) -> Result<(Macaroon, Arc<RootRecord>), LdkServerError> {
		let invalid = || auth_error("Invalid macaroon credentials");
		let macaroon = Macaroon::from_hex(token).map_err(|_| invalid())?;
		let id = std::str::from_utf8(macaroon.identifier()).map_err(|_| invalid())?;
		let record = self
			.roots
			.read()
			.map_err(|_| store_lock_error())?
			.get(id)
			.cloned()
			.ok_or_else(invalid)?;
		let root = Vec::<u8>::from_hex(&record.secret).map_err(|_| invalid())?;
		if !macaroon.verify_signature(&root) {
			return Err(invalid());
		}
		Ok((macaroon, record))
	}

	pub(crate) fn authenticate_request(
		&self, method: &str, auth_header: Option<&str>,
	) -> Result<PendingMacaroonRequest, LdkServerError> {
		let token = auth_header.ok_or_else(|| auth_error("Missing macaroon credentials"))?;
		let (macaroon, record) = self.verify_token(token)?;
		let (request_caveat, restrictions) = macaroon
			.caveats()
			.split_last()
			.ok_or_else(|| auth_error("Missing request binding caveat"))?;
		let binding = RequestBinding::parse(request_caveat).map_err(auth_error)?;
		if binding.method != method {
			return Err(auth_error("Macaroon request method does not match"));
		}
		check_request_timestamp(binding.timestamp, unix_time()?)?;
		// Only the final request proof is excluded from delegation. A request proof in
		// any preceding position or a stored root is an unknown condition and is denied.
		let info = Self::authenticate_caveats(&record, restrictions, method)?;
		Ok(PendingMacaroonRequest { info, binding })
	}

	pub(crate) fn finish_request(
		&self, request: PendingMacaroonRequest, method: &str, body: &[u8],
	) -> Result<Arc<MacaroonInfo>, LdkServerError> {
		self.finish_request_at(request, method, body, unix_time()?)
	}

	fn finish_request_at(
		&self, request: PendingMacaroonRequest, method: &str, body: &[u8], now: u64,
	) -> Result<Arc<MacaroonInfo>, LdkServerError> {
		check_request_timestamp(request.binding.timestamp, now)?;
		if request.binding.method != method || !request.binding.matches_body(body) {
			return Err(auth_error("Macaroon request body or method does not match"));
		}
		// Reading the body may take time. Recheck revocation and caveat expiry before
		// admitting the request, including before opening an event subscription.
		if !self.roots.read().map_err(|_| store_lock_error())?.contains_key(&request.info.id) {
			return Err(auth_error("Invalid macaroon credentials"));
		}
		let mut permissions = request.info.permissions.clone();
		for caveat in &request.info.caveats {
			check_caveat_at(caveat, method, &mut permissions, now)?;
		}
		Ok(request.info)
	}

	fn authenticate_caveats(
		record: &RootRecord, caveats: &[Vec<u8>], method: &str,
	) -> Result<Arc<MacaroonInfo>, LdkServerError> {
		let mut info = (*record.info).clone();
		// Report and inherit every effective limit, including limits added to root files.
		// All supported conditions are idempotent and their order does not affect access.
		// Thus we can deduplicate repeats without dropping restrictions during delegation.
		let mut seen = BTreeSet::new();
		info.caveats.retain(|caveat| seen.insert(caveat.clone()));
		for caveat in caveats {
			let caveat = String::from_utf8(caveat.clone())
				.map_err(|_| authorization_error("Invalid macaroon caveat"))?;
			if seen.insert(caveat.clone()) {
				info.caveats.push(caveat);
			}
		}
		for caveat in &info.caveats {
			check_caveat(caveat, method, &mut info.permissions)?;
		}
		Ok(Arc::new(info))
	}

	// Call management operations from a blocking thread. Authentication never takes this mutex.
	pub(crate) fn create_root(
		&self, name: &str, permissions: Vec<String>, issuer: &MacaroonInfo,
	) -> Result<CreatedMacaroon, LdkServerError> {
		self.create_root_with_writer(name, permissions, issuer, write_root_file)
	}

	fn create_root_with_writer(
		&self, name: &str, permissions: Vec<String>, issuer: &MacaroonInfo,
		write: impl FnOnce(&Path, &MacaroonInfo, &str) -> io::Result<()>,
	) -> Result<CreatedMacaroon, LdkServerError> {
		validate_name(name)?;
		let permissions = validate_permissions(permissions).map_err(invalid_request)?;
		let _management = self.management.lock().map_err(|_| store_lock_error())?;
		let issuer_permissions = management_permissions(issuer, CREATE_MACAROON_PATH)?;
		let secret = generate_secret().map_err(internal_error)?;
		let info = MacaroonInfo {
			id: compute_root_id(&secret),
			name: name.to_string(),
			permissions,
			caveats: issuer.caveats.clone(),
		};
		let token = mint_token(&info, &secret).map_err(invalid_request)?;
		{
			let roots = self.roots.read().map_err(|_| store_lock_error())?;
			if !roots.contains_key(&issuer.id) {
				return Err(auth_error("Invalid credentials"));
			}
			if roots.values().any(|record| record.info.name == name) {
				return Err(invalid_request(format!("macaroon name already exists: {name}")));
			}
			if !issuer_permissions.contains(ADMIN_PERMISSION)
				&& info
					.permissions
					.iter()
					.any(|permission| !issuer_permissions.contains(permission))
			{
				return Err(authorization_error(
					"Cannot grant a permission that the calling root does not have",
				));
			}
			if roots.contains_key(&info.id) {
				return Err(internal_error("Generated a duplicate macaroon ID"));
			}
		}
		let path = self.directory.join(format!("{}.toml", info.id));
		write(&path, &info, &secret).map_err(internal_error)?;
		let record =
			Arc::new(RootRecord { info: Arc::new(info.clone()), secret: secret.clone(), path });
		self.roots.write().map_err(|_| store_lock_error())?.insert(info.id.clone(), record);
		log::info!(
			"Created macaroon: issuer={} id={} name={} permissions={:?}",
			issuer.id,
			info.id,
			info.name,
			info.permissions
		);
		Ok(CreatedMacaroon { info, token })
	}

	pub(crate) fn list_roots(&self) -> Result<Vec<MacaroonInfo>, LdkServerError> {
		let records = self.roots.read().map_err(|_| store_lock_error())?;
		let mut roots: Vec<_> = records.values().map(|record| (*record.info).clone()).collect();
		roots.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
		Ok(roots)
	}

	pub(crate) fn revoke_root(
		&self, id: &str, issuer: &MacaroonInfo,
	) -> Result<(), LdkServerError> {
		if !is_hex(id, 32) {
			return Err(invalid_request(
				"macaroon ID must contain exactly 32 hexadecimal characters",
			));
		}
		let id = id.to_ascii_lowercase();
		let _management = self.management.lock().map_err(|_| store_lock_error())?;
		let issuer_permissions = management_permissions(issuer, REVOKE_MACAROON_PATH)?;
		let roots = self.roots.read().map_err(|_| store_lock_error())?;
		if !roots.contains_key(&issuer.id) {
			return Err(auth_error("Invalid credentials"));
		}
		let record =
			roots.get(&id).ok_or_else(|| invalid_request(format!("Unknown macaroon ID: {id}")))?;
		if !issuer_permissions.contains(ADMIN_PERMISSION)
			&& (record.info.is_admin()
				|| record
					.info
					.permissions
					.iter()
					.any(|permission| !issuer_permissions.contains(permission)))
		{
			return Err(authorization_error(
				"Cannot revoke a root with permissions that the calling root does not have",
			));
		}
		if is_unrestricted_admin(&record.info)
			&& roots.values().filter(|record| is_unrestricted_admin(&record.info)).count() == 1
		{
			return Err(invalid_request("Cannot revoke the final admin macaroon"));
		}

		let path = record.path.clone();
		let info = Arc::clone(&record.info);
		drop(roots);
		match fs::remove_file(path) {
			Ok(()) => {},
			// The file may have been deleted manually; still revoke the root from memory.
			Err(error) if error.kind() == io::ErrorKind::NotFound => {},
			Err(error) => return Err(internal_error(error)),
		}
		self.roots.write().map_err(|_| store_lock_error())?.remove(&id);
		File::open(&self.directory)
			.and_then(|directory| directory.sync_all())
			.map_err(internal_error)?;
		log::info!(
			"Revoked macaroon: issuer={} id={} name={} permissions={:?}",
			issuer.id,
			info.id,
			info.name,
			info.permissions
		);
		Ok(())
	}
}

#[cfg(test)]
pub(crate) mod test_util;

#[cfg(test)]
mod tests;
