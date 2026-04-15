# LDK Server

**LDK Server** is a fully-functional Lightning node in daemon form, built on top of
[LDK Node](https://github.com/lightningdevkit/ldk-node), which itself provides a powerful abstraction over the
[Lightning Development Kit (LDK)](https://github.com/lightningdevkit/rust-lightning) and uses a built-in
[Bitcoin Development Kit (BDK)](https://bitcoindevkit.org/) wallet.

The primary goal of LDK Server is to provide an efficient, stable, and API-first solution for deploying and managing
a Lightning Network node. With its streamlined setup, LDK Server enables users to easily set up, configure, and run
a Lightning node while exposing a robust, language-agnostic API via [Protocol Buffers (Protobuf)](https://protobuf.dev/).

## Workspace Crates

- `ldk-server`: daemon that runs the Lightning node and exposes the API
- `ldk-server-cli`: CLI client for the server API
- `ldk-server-client`: Rust client library for authenticated TLS gRPC calls
- `ldk-server-grpc`: generated protobuf and shared gRPC types
- `ldk-server-mcp`: stdio MCP bridge exposing unary `ldk-server` RPCs as MCP tools

### Features

- **Out-of-the-Box Lightning Node**:
    - Deploy a Lightning Network node with minimal configuration, no coding required.

- **API-First Design**:
    - Exposes a well-defined gRPC API using Protobuf, allowing seamless integration with any language.

- **Powered by LDK**:
    - Built on top of LDK-Node, leveraging the modular, reliable, and high-performance architecture of LDK.

- **Effortless Integration**:
    - Ideal for embedding Lightning functionality into payment processors, self-hosted nodes, custodial wallets, or other Lightning-enabled
      applications.

### Project Status

**Work in Progress**:
- APIs are under development. Expect breaking changes as the project evolves.
- Not tested for production use.
- We welcome your feedback and contributions to help shape the future of LDK Server!

### Quick Start

```bash
git clone https://github.com/lightningdevkit/ldk-server.git
cd ldk-server
cargo build --release
cp contrib/ldk-server-config.toml my-config.toml  # edit with your settings
./target/release/ldk-server my-config.toml
```

See [Getting Started](docs/getting-started.md) for a full walkthrough.

### Documentation

| Document | Description |
|----------|-------------|
| [Getting Started](docs/getting-started.md) | Install, configure, and run your first node |
| [Configuration](docs/configuration.md) | All config options, environment variables, and Bitcoin backend tradeoffs |
| [API Guide](docs/api-guide.md) | gRPC transport, authentication, and endpoint reference |
| [Tor](docs/tor.md) | Connecting to and receiving connections over Tor |
| [Operations](docs/operations.md) | Production deployment, backups, and monitoring |

### API

The canonical API definitions are in [`ldk-server-grpc/src/proto/`](ldk-server-grpc/src/proto/). A ready-made
Rust client library is provided in [`ldk-server-client/`](ldk-server-client/).

### MCP Bridge

The workspace also includes `ldk-server-mcp`, a stdio [Model Context Protocol](https://spec.modelcontextprotocol.io/) server
that lets MCP-compatible clients call the unary `ldk-server` RPC surface as tools.

Run it directly from the workspace:
```bash
cargo run -p ldk-server-mcp -- --config /path/to/config.toml
```

It is covered by both crate-local tests and an `e2e-tests` sanity suite against a live `ldk-server` instance.

### Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on building, testing, code style, and development workflow.
