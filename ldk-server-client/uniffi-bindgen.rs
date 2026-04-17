// Entry point for the uniffi-bindgen CLI, used to generate foreign-language
// bindings (e.g. Kotlin, Swift) from this crate's UniFFI-exported interface.
//
// Build it with `cargo build --features uniffi-cli --bin uniffi-bindgen` and
// invoke via e.g.
//     cargo run --features uniffi-cli --bin uniffi-bindgen -- \
//         generate --library target/<triple>/release/libldk_server_client.so \
//         --language kotlin --out-dir <out>
fn main() {
	uniffi::uniffi_bindgen_main()
}
