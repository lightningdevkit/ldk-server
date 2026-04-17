#!/usr/bin/env bash
#
# Cross-compiles `ldk-server-client` for the three Android ABIs we ship
# (arm64-v8a, armeabi-v7a, x86_64) and generates Kotlin bindings from the
# resulting cdylib using library mode.
#
# Outputs (relative to the workspace root, i.e. ../ldk-server):
#   target/<triple>/release/libldk_server_client.so
#   $OUT_DIR/kotlin/uniffi/ldk_server_client/ldk_server_client.kt
#   $OUT_DIR/jniLibs/<abi>/libldk_server_client.so
#
# Prerequisites (see README):
#   * Android SDK + NDK r27+ installed, with ANDROID_NDK_ROOT exported.
#   * Rust 1.93+ with Android targets (aarch64/armv7/x86_64-linux-android).
#   * cargo-ndk installed (`cargo install cargo-ndk`).
#
# Usage:
#   OUT_DIR=../ldk-server-remote/android/app/src/main \
#       ./scripts/uniffi_bindgen_generate_kotlin_android.sh
#
# If OUT_DIR is unset, files land in ./target/android-bindings/.

set -euo pipefail

# Resolve to the workspace root (this script lives in <root>/scripts).
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
WORKSPACE_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$WORKSPACE_ROOT"

OUT_DIR="${OUT_DIR:-$WORKSPACE_ROOT/target/android-bindings}"
KOTLIN_OUT="$OUT_DIR/kotlin"
JNI_LIBS_OUT="$OUT_DIR/jniLibs"

if [[ -z "${ANDROID_NDK_ROOT:-}" && -z "${NDK_HOME:-}" ]]; then
	cat >&2 <<'EOF'
error: ANDROID_NDK_ROOT (or NDK_HOME) is not set.
Install the NDK via `sdkmanager "ndk;27.0.12077973"` and export
    ANDROID_NDK_ROOT=$ANDROID_HOME/ndk/27.0.12077973
before re-running this script.
EOF
	exit 1
fi

if ! command -v cargo-ndk > /dev/null 2>&1; then
	echo "error: cargo-ndk not found. Install with: cargo install cargo-ndk" >&2
	exit 1
fi

# Android targets (ABI-named). Keep these aligned with the ABI folders under `jniLibs/`.
TARGETS=(arm64-v8a armeabi-v7a x86_64)

echo "[1/3] Cross-compiling ldk-server-client for Android (${TARGETS[*]})..."
cargo ndk \
	--manifest-path "$WORKSPACE_ROOT/ldk-server-client/Cargo.toml" \
	$(printf -- "-t %s " "${TARGETS[@]}") \
	--platform 24 \
	build --release --features uniffi

# Pick any one of the compiled .so files for library-mode bindings extraction;
# the metadata is platform-agnostic.
LIBRARY_PATH="$WORKSPACE_ROOT/target/x86_64-linux-android/release/libldk_server_client.so"

if [[ ! -f "$LIBRARY_PATH" ]]; then
	echo "error: expected library not found at $LIBRARY_PATH" >&2
	exit 1
fi

echo "[2/3] Generating Kotlin bindings..."
mkdir -p "$KOTLIN_OUT"
cargo run --manifest-path "$WORKSPACE_ROOT/ldk-server-client/Cargo.toml" \
	--features uniffi-cli --bin uniffi-bindgen -- \
	generate \
	--library "$LIBRARY_PATH" \
	--language kotlin \
	--config "$WORKSPACE_ROOT/ldk-server-client/uniffi-android.toml" \
	--out-dir "$KOTLIN_OUT"

echo "[3/3] Copying native libs into $JNI_LIBS_OUT..."
# Map cargo's target triples back to Android ABI directory names.
declare -A TRIPLES_TO_ABI=(
	[aarch64-linux-android]=arm64-v8a
	[armv7-linux-androideabi]=armeabi-v7a
	[x86_64-linux-android]=x86_64
)
for triple in "${!TRIPLES_TO_ABI[@]}"; do
	abi="${TRIPLES_TO_ABI[$triple]}"
	src="$WORKSPACE_ROOT/target/$triple/release/libldk_server_client.so"
	dst="$JNI_LIBS_OUT/$abi"
	mkdir -p "$dst"
	cp "$src" "$dst/libldk_server_client.so"
done

echo
echo "Done. Kotlin:   $KOTLIN_OUT"
echo "      jniLibs:  $JNI_LIBS_OUT"
