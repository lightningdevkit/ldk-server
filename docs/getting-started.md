# Getting Started

This guide walks you through building, configuring, and running your first LDK Server node.

## Prerequisites

- **Rust** 1.85.0 or later
- **A Bitcoin chain backend** (one of):
    - [Bitcoin Core](https://bitcoincore.org/) (bitcoind) with RPC enabled
    - An [Electrum](https://electrum.org/) server
    - An [Esplora](https://github.com/Blockstream/esplora) API endpoint

No other external dependencies are required.

## Build

```bash
git clone https://github.com/lightningdevkit/ldk-server.git
cd ldk-server
cargo build --release
```

The binaries are placed in `target/release/`:

- `ldk-server` (the node daemon)
- `ldk-server-cli` (the command-line client)

### Build Features

`experimental-lsps2-support` — Enables the LSPS2 liquidity service provider. **Experimental — for testing only.**
Requires `[liquidity.lsps2_service]` in config.

```bash
cargo build --release --features experimental-lsps2-support
```

## Configure

Copy the annotated config template and edit it:

```bash
cp contrib/ldk-server-config.toml my-config.toml
```

The only required decision is which Bitcoin backend to use. Keep **exactly one** of the
`[bitcoind]`, `[electrum]`, or `[esplora]` sections and remove the others.

**Minimal regtest example** (using Bitcoin Core):

```toml
[node]
network = "regtest"

[bitcoind]
rpc_address = "127.0.0.1:18443"
rpc_user = "user"
rpc_password = "pass"
```

Everything else has sensible defaults. See [Configuration](configuration.md) for the full
reference.

## Start the Server

```bash
./target/release/ldk-server my-config.toml
```

On first startup, watch the logs for:

```
gRPC service listening on 127.0.0.1:3536
NODE_URI: <node_id>@<address>
```

Two files are auto-generated on first run:

| File            | Location                          | Purpose                                  |
|-----------------|-----------------------------------|------------------------------------------|
| API key         | `<storage_dir>/<network>/api_key` | 32-byte random key (stored as raw bytes) |
| TLS certificate | `<storage_dir>/tls.crt`           | Self-signed ECDSA P-256 certificate      |

The default storage directory is `~/.ldk-server/` on Linux and
`~/Library/Application Support/ldk-server/` on macOS.

### Reading the API Key

The API key file contains raw bytes. To get the hex string the CLI and client library expect:

```bash
xxd -p -c 64 ~/.ldk-server/bitcoin/api_key
```

## First Commands

If the CLI and server share the same machine and use the default storage directory, the CLI
auto-discovers the API key and TLS certificate, so no flags are needed:

```bash
# Check the node is running
ldk-server-cli get-node-info

# Generate an on-chain funding address
ldk-server-cli onchain-receive

# Check balances
ldk-server-cli get-balances
```

When running on a different machine or using a non-default storage path, pass the connection
details explicitly:

```bash
ldk-server-cli \
  --base-url localhost:3536 \
  --api-key <hex_api_key> \
  --tls-cert /path/to/tls.crt \
  get-node-info
```

## CLI Tips

### Amount Syntax

Commands that accept amounts support `sat` and `msat` suffixes:

```bash
ldk-server-cli bolt11-receive --amount 50000sat
ldk-server-cli bolt11-receive --amount 50000000msat  # same as above
```

### Shell Completions

Generate completions for your shell:

```bash
# Bash (add to ~/.bashrc)
eval "$(ldk-server-cli completions bash)"

# Zsh (add to ~/.zshrc)
eval "$(ldk-server-cli completions zsh)"

# Fish (add to ~/.config/fish/config.fish)
ldk-server-cli completions fish | source
```

PowerShell and Elvish are also supported. Run `ldk-server-cli completions --help` for details.

### Per-Command Help

Every command supports `--help` for detailed argument descriptions:

```bash
ldk-server-cli open-channel --help
```

## Next Steps

- [Configuration](configuration.md): all config options, environment variables, and Bitcoin backend tradeoffs
- [API Guide](api-guide.md): gRPC transport, authentication, and endpoint reference
- [Operations](operations.md): production deployment, backups, and monitoring
