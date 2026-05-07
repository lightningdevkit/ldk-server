use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
	let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
	let profile = env::var("PROFILE").unwrap();

	let workspace_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
		.parent()
		.expect("e2e-tests must be inside workspace")
		.to_path_buf();

	let outer_target_dir = env::var_os("CARGO_TARGET_DIR")
		.map(PathBuf::from)
		.map(|path| if path.is_absolute() { path } else { workspace_root.join(path) })
		.unwrap_or_else(|| workspace_root.join("target"));

	// Use a separate target directory so the inner cargo build doesn't deadlock
	// waiting for the build directory lock held by the outer cargo.
	let target_dir = outer_target_dir.join("e2e-deps");

	let status = Command::new(&cargo)
		.args([
			"build",
			"-p",
			"ldk-server",
			"--features",
			"experimental-lsps2-support",
			"-p",
			"ldk-server-cli",
			"-p",
			"ldk-server-mcp",
		])
		.current_dir(&workspace_root)
		.env("CARGO_TARGET_DIR", &target_dir)
		.env_remove("CARGO_ENCODED_RUSTFLAGS")
		.status()
		.expect("failed to run cargo build");

	assert!(status.success(), "cargo build of ldk-server / ldk-server-cli / ldk-server-mcp failed");

	let bin_dir = target_dir.join(&profile);
	let server_bin = bin_dir.join("ldk-server");
	let cli_bin = bin_dir.join("ldk-server-cli");
	let mcp_bin = bin_dir.join("ldk-server-mcp");

	println!("cargo:rustc-env=LDK_SERVER_BIN={}", server_bin.display());
	println!("cargo:rustc-env=LDK_SERVER_CLI_BIN={}", cli_bin.display());
	println!("cargo:rustc-env=LDK_SERVER_MCP_BIN={}", mcp_bin.display());

	// Rebuild when server or CLI source changes
	println!("cargo:rerun-if-changed=../ldk-server/src");
	println!("cargo:rerun-if-changed=../ldk-server/Cargo.toml");
	println!("cargo:rerun-if-changed=../ldk-server-cli/src");
	println!("cargo:rerun-if-changed=../ldk-server-cli/Cargo.toml");
	println!("cargo:rerun-if-changed=../ldk-server-grpc/src");
	println!("cargo:rerun-if-changed=../ldk-server-grpc/Cargo.toml");
	println!("cargo:rerun-if-changed=../ldk-server-mcp/src");
	println!("cargo:rerun-if-changed=../ldk-server-mcp/Cargo.toml");
}
