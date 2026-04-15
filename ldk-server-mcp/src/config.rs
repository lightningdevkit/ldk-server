// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::path::PathBuf;

use ldk_server_client::config::{
	get_default_config_path, load_config, resolve_api_key, resolve_base_url, resolve_cert_path,
};

pub struct ResolvedConfig {
	pub base_url: String,
	pub api_key: String,
	pub tls_cert_pem: Vec<u8>,
}

pub fn resolve_config(config_path: Option<String>) -> Result<ResolvedConfig, String> {
	let env_base_url = std::env::var("LDK_BASE_URL").ok();
	let env_api_key = std::env::var("LDK_API_KEY").ok();
	let env_tls_cert_path = std::env::var("LDK_TLS_CERT_PATH").ok().map(PathBuf::from);
	let env_overrides_complete =
		env_base_url.is_some() && env_api_key.is_some() && env_tls_cert_path.is_some();

	let explicit_config_path = config_path.map(PathBuf::from);
	let config_path = explicit_config_path.clone().or_else(get_default_config_path);
	let config = match config_path {
		Some(ref path)
			if path.exists() && (explicit_config_path.is_some() || !env_overrides_complete) =>
		{
			Some(load_config(path)?)
		},
		_ => None,
	};

	let base_url = resolve_base_url(env_base_url, config.as_ref());

	let api_key = resolve_api_key(env_api_key, config.as_ref()).ok_or_else(
		|| "API key not provided. Set LDK_API_KEY or ensure the api_key file exists at ~/.ldk-server/[network]/api_key".to_string()
	)?;

	let tls_cert_path = resolve_cert_path(env_tls_cert_path, config.as_ref()).ok_or_else(|| {
		"TLS cert path not provided. Set LDK_TLS_CERT_PATH or ensure config file exists at ~/.ldk-server/config.toml"
			.to_string()
	})?;

	let tls_cert_pem = std::fs::read(&tls_cert_path).map_err(|e| {
		format!("Failed to read server certificate file '{}': {}", tls_cert_path.display(), e)
	})?;

	Ok(ResolvedConfig { base_url, api_key, tls_cert_pem })
}

#[cfg(test)]
mod tests {
	use super::resolve_config;
	use ldk_server_client::config::{get_default_config_path, DEFAULT_GRPC_SERVICE_ADDRESS};
	use std::path::PathBuf;
	use std::sync::Mutex;

	// Tests that call resolve_config manipulate process-global environment
	// variables, so they must not run in parallel.
	static ENV_LOCK: Mutex<()> = Mutex::new(());

	#[cfg(target_os = "windows")]
	fn set_default_data_dir(temp_dir: &std::path::Path) -> (String, Option<String>) {
		let old_value = std::env::var("APPDATA").ok();
		std::env::set_var("APPDATA", temp_dir);
		("APPDATA".to_string(), old_value)
	}

	#[cfg(not(target_os = "windows"))]
	fn set_default_data_dir(temp_dir: &std::path::Path) -> (String, Option<String>) {
		let old_value = std::env::var("HOME").ok();
		std::env::set_var("HOME", temp_dir);
		("HOME".to_string(), old_value)
	}

	fn restore_env_var(name: &str, value: Option<String>) {
		match value {
			Some(value) => std::env::set_var(name, value),
			None => std::env::remove_var(name),
		}
	}

	#[test]
	fn resolve_config_uses_grpc_service_address_from_config() {
		let _lock = ENV_LOCK.lock().unwrap();

		let temp_dir =
			std::env::temp_dir().join(format!("ldk-server-mcp-config-test-{}", std::process::id()));
		std::fs::create_dir_all(&temp_dir).unwrap();

		let config_path = temp_dir.join("config.toml");
		let cert_path = temp_dir.join("tls.crt");
		std::fs::write(&cert_path, b"test-cert").unwrap();
		std::fs::write(
			&config_path,
			format!(
				r#"
					[node]
					network = "regtest"
					grpc_service_address = "127.0.0.1:4242"

					[tls]
					cert_path = "{}"
				"#,
				cert_path.display()
			),
		)
		.unwrap();

		std::env::set_var("LDK_API_KEY", "deadbeef");
		std::env::set_var("LDK_TLS_CERT_PATH", &cert_path);
		std::env::remove_var("LDK_BASE_URL");
		let resolved = resolve_config(Some(config_path.display().to_string())).unwrap();
		std::env::remove_var("LDK_API_KEY");
		std::env::remove_var("LDK_TLS_CERT_PATH");

		assert_eq!(resolved.base_url, "127.0.0.1:4242");
		assert_eq!(resolved.api_key, "deadbeef");
		assert_eq!(resolved.tls_cert_pem, b"test-cert");

		std::fs::remove_dir_all(temp_dir).unwrap();
	}

	#[test]
	fn resolve_config_falls_back_to_default_grpc_address() {
		let _lock = ENV_LOCK.lock().unwrap();

		let temp_dir = std::env::temp_dir()
			.join(format!("ldk-server-mcp-config-fallback-{}", std::process::id()));
		std::fs::create_dir_all(&temp_dir).unwrap();

		let cert_path = temp_dir.join("tls.crt");
		std::fs::write(&cert_path, b"test-cert").unwrap();

		// No config file, no LDK_BASE_URL — should fall back to default
		std::env::set_var("LDK_API_KEY", "deadbeef");
		std::env::set_var("LDK_TLS_CERT_PATH", &cert_path);
		std::env::remove_var("LDK_BASE_URL");
		let resolved =
			resolve_config(Some(temp_dir.join("nonexistent.toml").display().to_string())).unwrap();
		std::env::remove_var("LDK_API_KEY");
		std::env::remove_var("LDK_TLS_CERT_PATH");

		assert_eq!(resolved.base_url, DEFAULT_GRPC_SERVICE_ADDRESS);

		std::fs::remove_dir_all(temp_dir).unwrap();
	}

	#[test]
	fn resolve_config_ignores_malformed_default_config_when_env_complete() {
		let _lock = ENV_LOCK.lock().unwrap();

		let temp_dir = std::env::temp_dir()
			.join(format!("ldk-server-mcp-env-overrides-{}", std::process::id()));
		std::fs::create_dir_all(&temp_dir).unwrap();
		let (default_dir_env_var, old_default_dir) = set_default_data_dir(&temp_dir);

		let default_config_path = get_default_config_path().unwrap();
		std::fs::create_dir_all(default_config_path.parent().unwrap()).unwrap();
		std::fs::write(&default_config_path, "not valid toml = [").unwrap();

		let cert_path: PathBuf = temp_dir.join("env.crt");
		std::fs::write(&cert_path, b"env-cert").unwrap();

		std::env::set_var("LDK_BASE_URL", "127.0.0.1:4242");
		std::env::set_var("LDK_API_KEY", "deadbeef");
		std::env::set_var("LDK_TLS_CERT_PATH", &cert_path);

		let resolved = resolve_config(None).unwrap();

		std::env::remove_var("LDK_BASE_URL");
		std::env::remove_var("LDK_API_KEY");
		std::env::remove_var("LDK_TLS_CERT_PATH");
		restore_env_var(&default_dir_env_var, old_default_dir);

		assert_eq!(resolved.base_url, "127.0.0.1:4242");
		assert_eq!(resolved.api_key, "deadbeef");
		assert_eq!(resolved.tls_cert_pem, b"env-cert");

		std::fs::remove_dir_all(temp_dir).unwrap();
	}

	#[test]
	fn resolve_config_uses_storage_dir_for_credentials() {
		let _lock = ENV_LOCK.lock().unwrap();

		let temp_dir =
			std::env::temp_dir().join(format!("ldk-server-mcp-storage-dir-{}", std::process::id()));
		std::fs::create_dir_all(temp_dir.join("regtest")).unwrap();

		let config_path = temp_dir.join("config.toml");
		let custom_storage = temp_dir.join("custom-storage");
		std::fs::create_dir_all(custom_storage.join("regtest")).unwrap();

		let cert_path = custom_storage.join("tls.crt");
		std::fs::write(&cert_path, b"storage-cert").unwrap();
		std::fs::write(custom_storage.join("regtest").join("api_key"), [0xAB, 0xCD]).unwrap();

		std::fs::write(
			&config_path,
			format!(
				r#"
					[node]
					network = "regtest"

					[storage.disk]
					dir_path = "{}"
				"#,
				custom_storage.display()
			),
		)
		.unwrap();

		std::env::remove_var("LDK_API_KEY");
		std::env::remove_var("LDK_TLS_CERT_PATH");
		std::env::remove_var("LDK_BASE_URL");
		let resolved = resolve_config(Some(config_path.display().to_string())).unwrap();

		assert_eq!(resolved.base_url, DEFAULT_GRPC_SERVICE_ADDRESS);
		assert_eq!(resolved.api_key, "abcd");
		assert_eq!(resolved.tls_cert_pem, b"storage-cert");

		std::fs::remove_dir_all(temp_dir).unwrap();
	}
}
