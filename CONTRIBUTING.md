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
cargo run --bin ldk-server ./ldk-server/ldk-server-config.toml
```

## Testing

```bash
cargo test                     # Run all tests
cargo test --all-features      # Run tests with all features
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
RUSTFLAGS="--cfg genproto" cargo build -p ldk-server-protos
cargo fmt --all
```

## Adding a New API Endpoint

1. Define request/response messages in `ldk-server-protos/src/proto/api.proto`
2. Regenerate protos (see above)
3. Create handler in `ldk-server/src/api/` (follow existing patterns)
4. Add route in `ldk-server/src/service.rs`
5. Add CLI command in `ldk-server-cli/src/main.rs`

## Configuration

- Config template with all options: `ldk-server/ldk-server-config.toml`
- When updating config options, also update the tests in `ldk-server/src/util/config.rs`

## Before Submitting

- Ensure all tests pass
- Ensure all lints are fixed
- Run `cargo fmt --all`
- Please disclose the use of any AI tools in commit messages and PR descriptions
