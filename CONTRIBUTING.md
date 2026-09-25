# Contributing to LDK Server

Contributions are welcome and encouraged! Whether you're fixing bugs, adding features, improving documentation, or
helping with testing, we appreciate your help!

## Building

```bash
cargo build                    # Build all crates
cargo build --release          # Production build (LTO enabled)
```

## Running

```bash
cargo run --bin ldk-server ./contrib/ldk-server-config.toml
```

## Testing

```bash
cargo test                     # Run workspace tests
cargo test --all-features      # Run workspace tests with all features
```

Run the end-to-end tests from their separate workspace:

```bash
cargo test --manifest-path e2e-tests/Cargo.toml -- --test-threads=4
```

## Code Quality

```bash
cargo fmt --all                                                      # Format code
cargo fmt --all -- --check                                           # Check formatting
cargo clippy --all-features -- -D warnings -A clippy::drop_non_drop  # Lint (CI uses this on MSRV)
```

## Code Style

- MSRV: Rust 1.85.0
- Hard tabs, max width 100 chars
- Imports grouped: std, external crates, local crates

## Protocol Buffer Generation

```bash
RUSTFLAGS="--cfg genproto" cargo build -p ldk-server-grpc
cargo fmt --all
```

## Adding a New API Endpoint

1. Define request/response messages in `ldk-server-grpc/src/proto/api.proto`
2. Regenerate protos (see above)
3. Create handler in `ldk-server/src/api/` (follow existing patterns)
4. Add route in `ldk-server/src/service.rs`
5. Map the RPC to its required permission in `method_authorization` in `ldk-server/src/macaroons/authorization.rs`.
   Unmapped methods return `UNIMPLEMENTED`, even for admin tokens.
6. Add CLI command in `ldk-server-cli/src/main.rs`
7. For a unary RPC, add the MCP tool in `ldk-server-mcp/src/tools/` and update the tool list test
   in `ldk-server-mcp/tests/integration.rs`. Add a live test in `e2e-tests/tests/mcp.rs` if applicable.
8. Test allowed and denied requests, including admin access.

If the RPC needs a new permission, add it to `ldk-server-grpc/src/permissions.rs` and
`ALL_PERMISSIONS`. Update the presets that need it and add it to `docs/api-guide.md`.

## Configuration

- Config template with all options: `contrib/ldk-server-config.toml`
- When updating config options, also update the tests in `ldk-server/src/util/config.rs`

## Before Submitting

- Ensure all tests pass
- Ensure all lints are fixed
- Run `cargo fmt --all`
- Please disclose the use of any AI tools in commit messages and PR descriptions
