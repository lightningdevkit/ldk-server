// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use log::LevelFilter;
use serde::Deserialize;

/// CLI args. The single positional is the path to a TOML config file. All other
/// behavior is configured via the file.
#[derive(Parser, Debug)]
#[command(name = "ldk-server-mcp", about = "MCP gateway for LDK Server", version)]
pub struct ArgsConfig {
	/// Path to the TOML configuration file.
	pub config_file: PathBuf,
}

/// On-disk TOML schema. Mirrors the daemon's config style.
///
/// `deny_unknown_fields` is set on every section so a typo in `cert_path` (etc.)
/// surfaces as a parse error instead of silently using the default.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
	gateway: GatewaySection,
	daemon: DaemonSection,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GatewaySection {
	listen_addr: String,
	storage_dir: String,
	#[serde(default)]
	log_level: Option<String>,
	#[serde(default)]
	log_file_path: Option<String>,
	#[serde(default)]
	tls: Option<TlsSection>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TlsSection {
	#[serde(default)]
	cert_path: Option<String>,
	#[serde(default)]
	key_path: Option<String>,
	#[serde(default)]
	hosts: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DaemonSection {
	address: String,
	api_key_path: String,
	tls_cert_path: String,
}

/// Optional per-host TLS settings for the gateway. If both fields are `None`,
/// a self-signed certificate is auto-generated under `storage_dir`.
#[derive(Debug, Clone)]
pub struct TlsConfig {
	pub cert_path: Option<String>,
	pub key_path: Option<String>,
	pub hosts: Vec<String>,
}

/// Connection details for the upstream LDK Server daemon.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
	/// Bare host:port (the scheme is stripped if the user includes one).
	pub address: String,
	pub api_key_path: PathBuf,
	pub tls_cert_path: PathBuf,
}

/// Fully-validated configuration ready to drive the runtime.
#[derive(Debug, Clone)]
pub struct Config {
	pub listen_addr: SocketAddr,
	pub storage_dir: PathBuf,
	pub log_level: LevelFilter,
	pub log_file_path: PathBuf,
	pub tls_config: Option<TlsConfig>,
	pub daemon: DaemonConfig,
}

/// Loads and validates the configuration file referenced by `args`.
pub fn load_config(args: &ArgsConfig) -> Result<Config, String> {
	let raw = fs::read_to_string(&args.config_file)
		.map_err(|e| format!("Failed to read config file '{}': {e}", args.config_file.display()))?;
	let parsed: ConfigFile = toml::from_str(&raw).map_err(|e| {
		format!("Failed to parse config file '{}': {e}", args.config_file.display())
	})?;

	let listen_addr = parsed.gateway.listen_addr.parse::<SocketAddr>().map_err(|e| {
		format!("Invalid gateway.listen_addr '{}': {e}", parsed.gateway.listen_addr)
	})?;

	let storage_dir = PathBuf::from(&parsed.gateway.storage_dir);
	if storage_dir.as_os_str().is_empty() {
		return Err("gateway.storage_dir must be a non-empty path".to_string());
	}

	let log_level = parse_log_level(parsed.gateway.log_level.as_deref())?;
	let log_file_path = match parsed.gateway.log_file_path {
		Some(p) => PathBuf::from(p),
		None => storage_dir.join("ldk-server-mcp.log"),
	};

	if log_file_path == storage_dir {
		return Err("gateway.log_file_path cannot be the same as gateway.storage_dir".to_string());
	}

	let tls_config = parsed.gateway.tls.map(|t| TlsConfig {
		cert_path: t.cert_path,
		key_path: t.key_path,
		hosts: t.hosts,
	});

	let address = strip_scheme(&parsed.daemon.address).to_string();
	if address.is_empty() {
		return Err("daemon.address must include a host:port".to_string());
	}

	let api_key_path = PathBuf::from(&parsed.daemon.api_key_path);
	if !api_key_path.is_absolute() {
		// Daemons are typically managed via systemd units with absolute paths;
		// relative paths are almost always a misconfiguration.
		return Err(format!(
			"daemon.api_key_path must be an absolute path, got '{}'",
			api_key_path.display()
		));
	}

	let tls_cert_path = PathBuf::from(&parsed.daemon.tls_cert_path);
	if !tls_cert_path.is_absolute() {
		return Err(format!(
			"daemon.tls_cert_path must be an absolute path, got '{}'",
			tls_cert_path.display()
		));
	}

	Ok(Config {
		listen_addr,
		storage_dir,
		log_level,
		log_file_path,
		tls_config,
		daemon: DaemonConfig { address, api_key_path, tls_cert_path },
	})
}

fn parse_log_level(s: Option<&str>) -> Result<LevelFilter, String> {
	match s.map(str::to_ascii_lowercase).as_deref() {
		None | Some("info") => Ok(LevelFilter::Info),
		Some("trace") => Ok(LevelFilter::Trace),
		Some("debug") => Ok(LevelFilter::Debug),
		Some("warn") => Ok(LevelFilter::Warn),
		Some("error") => Ok(LevelFilter::Error),
		Some("off") => Ok(LevelFilter::Off),
		Some(other) => Err(format!("Invalid log_level '{other}'")),
	}
}

fn strip_scheme(s: &str) -> &str {
	s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).unwrap_or(s)
}

#[cfg(test)]
mod tests {
	use std::io::Write;

	use super::*;

	fn write_temp_config(contents: &str) -> PathBuf {
		let mut suffix_bytes = [0u8; 8];
		getrandom::getrandom(&mut suffix_bytes).unwrap();
		let suffix = u64::from_ne_bytes(suffix_bytes);
		let path = std::env::temp_dir().join(format!("ldk-server-mcp-test-{suffix}.toml"));
		let mut f = fs::File::create(&path).unwrap();
		f.write_all(contents.as_bytes()).unwrap();
		path
	}

	#[test]
	fn parses_minimal_config() {
		let path = write_temp_config(
			r#"
[gateway]
listen_addr = "127.0.0.1:3537"
storage_dir = "/var/lib/ldk-server-mcp"

[daemon]
address = "https://127.0.0.1:3536"
api_key_path = "/var/lib/ldk-server/bitcoin/api_key"
tls_cert_path = "/var/lib/ldk-server/tls.crt"
"#,
		);

		let args = ArgsConfig { config_file: path.clone() };
		let cfg = load_config(&args).unwrap();
		fs::remove_file(&path).ok();

		assert_eq!(cfg.listen_addr.to_string(), "127.0.0.1:3537");
		assert_eq!(cfg.log_level, LevelFilter::Info);
		assert_eq!(cfg.daemon.address, "127.0.0.1:3536");
		assert!(cfg.tls_config.is_none());
	}

	#[test]
	fn parses_full_config() {
		let path = write_temp_config(
			r#"
[gateway]
listen_addr = "0.0.0.0:8443"
storage_dir = "/srv/ldk-mcp"
log_level = "debug"
log_file_path = "/var/log/ldk-server-mcp.log"

[gateway.tls]
cert_path = "/etc/ldk-server-mcp/tls.crt"
key_path  = "/etc/ldk-server-mcp/tls.key"
hosts = ["mcp.example.com"]

[daemon]
address = "127.0.0.1:3536"
api_key_path = "/var/lib/ldk-server/bitcoin/api_key"
tls_cert_path = "/var/lib/ldk-server/tls.crt"
"#,
		);

		let args = ArgsConfig { config_file: path.clone() };
		let cfg = load_config(&args).unwrap();
		fs::remove_file(&path).ok();

		assert_eq!(cfg.log_level, LevelFilter::Debug);
		assert_eq!(cfg.log_file_path, PathBuf::from("/var/log/ldk-server-mcp.log"));
		let tls = cfg.tls_config.unwrap();
		assert_eq!(tls.cert_path.unwrap(), "/etc/ldk-server-mcp/tls.crt");
		assert_eq!(tls.hosts, vec!["mcp.example.com".to_string()]);
	}

	#[test]
	fn rejects_invalid_listen_addr() {
		let path = write_temp_config(
			r#"
[gateway]
listen_addr = "not-a-socket"
storage_dir = "/tmp"

[daemon]
address = "127.0.0.1:3536"
api_key_path = "/var/lib/ldk-server/bitcoin/api_key"
tls_cert_path = "/var/lib/ldk-server/tls.crt"
"#,
		);

		let args = ArgsConfig { config_file: path.clone() };
		let err = load_config(&args).unwrap_err();
		fs::remove_file(&path).ok();
		assert!(err.contains("listen_addr"), "unexpected error: {err}");
	}

	#[test]
	fn rejects_relative_api_key_path() {
		let path = write_temp_config(
			r#"
[gateway]
listen_addr = "127.0.0.1:3537"
storage_dir = "/tmp"

[daemon]
address = "127.0.0.1:3536"
api_key_path = "relative/path"
tls_cert_path = "/var/lib/ldk-server/tls.crt"
"#,
		);

		let args = ArgsConfig { config_file: path.clone() };
		let err = load_config(&args).unwrap_err();
		fs::remove_file(&path).ok();
		assert!(err.contains("absolute path"), "unexpected error: {err}");
	}

	#[test]
	fn strips_scheme_from_daemon_address() {
		assert_eq!(strip_scheme("https://localhost:3536"), "localhost:3536");
		assert_eq!(strip_scheme("http://localhost:3536"), "localhost:3536");
		assert_eq!(strip_scheme("localhost:3536"), "localhost:3536");
	}

	#[test]
	fn rejects_invalid_log_level() {
		assert!(parse_log_level(Some("verbose")).is_err());
	}

	#[test]
	fn rejects_unknown_top_level_field() {
		let path = write_temp_config(
			r#"
[gateway]
listen_addr = "127.0.0.1:3537"
storage_dir = "/tmp"

[daemon]
address = "127.0.0.1:3536"
api_key_path = "/var/lib/ldk-server/bitcoin/api_key"
tls_cert_path = "/var/lib/ldk-server/tls.crt"

[surprise]
hello = "world"
"#,
		);

		let args = ArgsConfig { config_file: path.clone() };
		let err = load_config(&args).unwrap_err();
		fs::remove_file(&path).ok();
		assert!(err.contains("unknown") || err.contains("surprise"), "unexpected error: {err}");
	}

	#[test]
	fn rejects_unknown_gateway_field() {
		let path = write_temp_config(
			r#"
[gateway]
listen_addr = "127.0.0.1:3537"
storage_dir = "/tmp"
typo_field = "oops"

[daemon]
address = "127.0.0.1:3536"
api_key_path = "/var/lib/ldk-server/bitcoin/api_key"
tls_cert_path = "/var/lib/ldk-server/tls.crt"
"#,
		);

		let args = ArgsConfig { config_file: path.clone() };
		let err = load_config(&args).unwrap_err();
		fs::remove_file(&path).ok();
		assert!(err.contains("unknown") || err.contains("typo_field"), "unexpected error: {err}");
	}
}
