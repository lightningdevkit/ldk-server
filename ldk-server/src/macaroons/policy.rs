// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Permission and caveat evaluation, independent of root storage.

use std::collections::BTreeSet;

use hex::FromHex;
use ldk_server_grpc::permissions::{
	ADMIN_PERMISSION, ALL_PERMISSIONS, MACAROONS_MANAGE_PERMISSION,
};
use ldk_server_macaroons::{Macaroon, REQUEST_TIMESTAMP_TOLERANCE_SECS};

use super::{auth_error, authorization_error, internal_error, invalid_request};
use crate::api::error::LdkServerError;

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

pub(super) fn mint_token(info: &MacaroonInfo, secret: &str) -> Result<String, &'static str> {
	let root = Vec::<u8>::from_hex(secret).map_err(|_| "Invalid macaroon root key")?;
	let mut macaroon = Macaroon::mint(&root, info.id.as_bytes())?;
	let permissions = info.permissions.iter().cloned().collect::<Vec<_>>().join(",");
	macaroon.attenuate(format!("permissions = {permissions}").as_bytes())?;
	for caveat in &info.caveats {
		macaroon.attenuate(caveat.as_bytes())?;
	}
	macaroon.check_request_capacity()?;
	Ok(macaroon.to_hex())
}

pub(super) fn unix_time() -> Result<u64, LdkServerError> {
	std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|time| time.as_secs())
		.map_err(internal_error)
}

pub(super) fn check_request_timestamp(timestamp: u64, now: u64) -> Result<(), LdkServerError> {
	if now.abs_diff(timestamp) > REQUEST_TIMESTAMP_TOLERANCE_SECS {
		return Err(auth_error("Macaroon request timestamp expired"));
	}
	Ok(())
}

pub(super) fn check_caveat(
	caveat: &str, method: &str, permissions: &mut BTreeSet<String>,
) -> Result<(), LdkServerError> {
	check_caveat_at(caveat, method, permissions, unix_time()?)
}

pub(super) fn check_caveat_at(
	caveat: &str, method: &str, permissions: &mut BTreeSet<String>, now: u64,
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

pub(super) fn is_unrestricted_admin(info: &MacaroonInfo) -> bool {
	info.is_admin() && info.caveats.iter().all(|c| c == "permissions = admin")
}

pub(super) fn validate_name(name: &str) -> Result<(), LdkServerError> {
	validate_name_value(name).map_err(invalid_request)
}

pub(super) fn validate_name_value(name: &str) -> Result<(), String> {
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

pub(super) fn validate_permissions(permissions: Vec<String>) -> Result<BTreeSet<String>, String> {
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

pub(super) fn management_permissions(
	issuer: &MacaroonInfo, method: &str,
) -> Result<BTreeSet<String>, LdkServerError> {
	if !issuer.allows(MACAROONS_MANAGE_PERMISSION) {
		return Err(authorization_error("Macaroon management permission required"));
	}
	let mut issuer_permissions = issuer.permissions.clone();
	for caveat in &issuer.caveats {
		check_caveat(caveat, method, &mut issuer_permissions)?;
	}
	if !issuer_permissions.contains(ADMIN_PERMISSION)
		&& !issuer_permissions.contains(MACAROONS_MANAGE_PERMISSION)
	{
		return Err(authorization_error("Macaroon management permission required"));
	}
	Ok(issuer_permissions)
}

#[cfg(test)]
mod tests {
	use ldk_server_grpc::permissions::NODE_READ_PERMISSION;

	use super::*;
	#[test]
	fn rejects_unknown_and_mixed_admin_permissions() {
		assert!(validate_permissions(vec!["unknown:permission".to_string()]).is_err());
		assert!(validate_permissions(vec![
			ADMIN_PERMISSION.to_string(),
			NODE_READ_PERMISSION.to_string(),
		])
		.is_err());
	}

	#[test]
	fn request_timestamp_tolerance_has_exact_bounds() {
		for timestamp in [940, 1000, 1060] {
			assert!(check_request_timestamp(timestamp, 1000).is_ok());
		}
		for timestamp in [0, 939, 1061, u64::MAX] {
			assert!(check_request_timestamp(timestamp, 1000).is_err());
		}
	}
}
