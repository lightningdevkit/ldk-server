# ldk-server-cli

Command-line client for interacting with a running [LDK Server](https://github.com/lightningdevkit/ldk-server)
node.

## Installation

```bash
cargo install ldk-server-cli --locked
```

Or build from the repository root:

```bash
cargo build --release -p ldk-server-cli
```

## Prerequisites

A running LDK Server instance. See the [Getting Started](../docs/getting-started.md) guide.

## Quick Start

If the CLI and server are on the same machine with default paths, no flags are needed:

```bash
ldk-server-cli get-node-info
ldk-server-cli onchain-receive
ldk-server-cli get-balances
```

When using custom paths or connecting remotely:

```bash
ldk-server-cli \
  --base-url localhost:3536 \
  --api-key <hex_api_key> \
  --tls-cert /path/to/tls.crt \
  get-node-info
```

## Documentation

- [Getting Started](../docs/getting-started.md): first-run walkthrough, shell completions, and CLI tips
- [API Guide](../docs/api-guide.md): gRPC API details and endpoint reference
