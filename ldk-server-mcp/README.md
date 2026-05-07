# ldk-server-mcp

An [MCP (Model Context Protocol)](https://spec.modelcontextprotocol.io/) server that exposes [LDK Server](https://github.com/lightningdevkit/ldk-server) operations as tools for AI agents. It communicates over JSON-RPC 2.0 via stdio and connects to an LDK Server instance over TLS using the [`ldk-server-client`](https://github.com/lightningdevkit/ldk-server/tree/main/ldk-server-client) library.

This crate lives inside the `ldk-server` workspace.

## Building

```bash
cargo build -p ldk-server-mcp --release
```

## Configuration

The server reads configuration in this precedence order (highest wins):

1. **Environment variables**: `LDK_BASE_URL`, `LDK_API_KEY`, `LDK_TLS_CERT_PATH`
2. **CLI argument**: `--config <path>` pointing to a TOML config file
3. **Default paths**: `~/.ldk-server/config.toml`, `~/.ldk-server/tls.crt`, `~/.ldk-server/{network}/api_key`

The TOML config format is the same as used by [`ldk-server-cli`](https://github.com/lightningdevkit/ldk-server/tree/main/ldk-server-cli):

```toml
[node]
grpc_service_address = "127.0.0.1:3536"
network = "signet"

[tls]
cert_path = "/path/to/tls.crt"
```

## Usage

### Standalone

```bash
export LDK_BASE_URL="localhost:3000"
export LDK_API_KEY="your_hex_encoded_api_key"
export LDK_TLS_CERT_PATH="/path/to/tls.crt"
cargo run -p ldk-server-mcp --release
```

Or using a config file:

```bash
cargo run -p ldk-server-mcp -- --config /path/to/config.toml
```

If `--config` is omitted, `ldk-server-mcp` falls back to the same default config path as
`ldk-server` and `ldk-server-cli`: `~/.ldk-server/config.toml`.

### With Claude Desktop

Add the following to your Claude Desktop MCP configuration (`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "ldk-server": {
      "command": "/path/to/ldk-server-mcp",
      "env": {
        "LDK_BASE_URL": "localhost:3000",
        "LDK_API_KEY": "your_hex_encoded_api_key",
        "LDK_TLS_CERT_PATH": "/path/to/tls.crt"
      }
    }
  }
}
```

### With Claude Code

Add to your Claude Code MCP settings (`.claude/settings.json`):

```json
{
  "mcpServers": {
    "ldk-server": {
      "command": "/path/to/ldk-server-mcp",
      "env": {
        "LDK_BASE_URL": "localhost:3000",
        "LDK_API_KEY": "your_hex_encoded_api_key",
        "LDK_TLS_CERT_PATH": "/path/to/tls.crt"
      }
    }
  }
}
```

## Available Tools

All unary LDK Server RPCs are exposed as MCP tools. Use `tools/list` to discover the current set.

Streaming RPCs such as `subscribe_events` and non-RPC HTTP endpoints such as `metrics` are not exposed as tools.

## MCP Protocol

- **Protocol version**: `2025-11-25`
- **Transport**: stdio (one JSON-RPC 2.0 message per line)
- **Methods**: `initialize`, `tools/list`, `tools/call`

## Testing

```bash
cargo test -p ldk-server-mcp

# MCP end-to-end sanity checks against a live ldk-server
cargo test --manifest-path e2e-tests/Cargo.toml mcp -- --nocapture
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
