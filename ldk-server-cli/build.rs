// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-env-changed=GIT_HASH");

	// Re-run the build script whenever the checked-out commit changes so the
	// embedded hash never goes stale. HEAD lives in the (possibly worktree-local)
	// git dir, while branch refs and packed-refs live in the common git dir.
	if let Some(git_dir) = git_output(&["rev-parse", "--git-dir"]) {
		watch_path(&Path::new(&git_dir).join("HEAD"));
	}
	if let Some(common_dir) = git_output(&["rev-parse", "--git-common-dir"]) {
		let common_dir = Path::new(&common_dir);
		watch_path(&common_dir.join("packed-refs"));
		// If HEAD points at a ref, watch that ref file too (it changes on commit).
		// Watch it even when it does not exist yet: packed refs become loose
		// files on the next commit, and Cargo can detect that creation.
		if let Some(ref_path) = git_output(&["symbolic-ref", "-q", "HEAD"]) {
			watch_path(&common_dir.join(ref_path));
		}
	}

	let git_hash = git_output(&["rev-parse", "HEAD"])
		.or_else(|| env::var("GIT_HASH").ok())
		.unwrap_or_else(|| "unknown".to_string());
	println!("cargo:rustc-env=GIT_HASH={git_hash}");
}

/// Runs `git` with the given args, returning the trimmed stdout on success or
/// `None` if git is unavailable, exits non-zero, or produces no output.
fn git_output(args: &[&str]) -> Option<String> {
	let output = Command::new("git").args(args).output().ok()?;
	if !output.status.success() {
		return None;
	}
	let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
	if stdout.is_empty() {
		None
	} else {
		Some(stdout)
	}
}

fn watch_path(path: &Path) {
	println!("cargo:rerun-if-changed={}", path.display());
}
