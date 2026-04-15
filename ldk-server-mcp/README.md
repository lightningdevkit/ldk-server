# ldk-server-mcp

An [MCP (Model Context Protocol)](https://spec.modelcontextprotocol.io/) server that exposes [LDK Server](https://github.com/lightningdevkit/ldk-server) operations as tools for AI agents. It communicates over JSON-RPC 2.0 via stdio and connects to an LDK Server instance over TLS using the [`ldk-server-client`](https://github.com/lightningdevkit/ldk-server/tree/main/ldk-server-client) library.

## Building

```bash
cargo build --release
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
./target/release/ldk-server-mcp
```

Or using a config file:

```bash
./target/release/ldk-server-mcp --config /path/to/config.toml
```

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

The server exposes 37 unary LDK Server RPCs as MCP tools.

Streaming RPCs such as `subscribe_events` and non-RPC HTTP endpoints such as `metrics` are not exposed as tools.

### Node
| Tool | Description |
|------|-------------|
| `get_node_info` | Retrieve node info including node_id, sync status, and best block |
| `get_balances` | Retrieve an overview of all known balances (on-chain and Lightning) |

### On-chain
| Tool | Description |
|------|-------------|
| `onchain_receive` | Generate a new on-chain Bitcoin funding address |
| `onchain_send` | Send an on-chain Bitcoin payment to an address |

### Payments
| Tool | Description |
|------|-------------|
| `bolt11_receive` | Create a BOLT11 Lightning invoice to receive a payment |
| `bolt11_receive_for_hash` | Create a BOLT11 Lightning invoice for a specific payment hash |
| `bolt11_claim_for_hash` | Manually claim a BOLT11 payment for a specific payment hash |
| `bolt11_fail_for_hash` | Manually fail a BOLT11 payment for a specific payment hash |
| `bolt11_receive_via_jit_channel` | Create a BOLT11 Lightning invoice to receive via an LSPS2 JIT channel |
| `bolt11_receive_variable_amount_via_jit_channel` | Create a variable-amount BOLT11 Lightning invoice to receive via an LSPS2 JIT channel |
| `bolt11_send` | Pay a BOLT11 Lightning invoice |
| `bolt12_receive` | Create a BOLT12 offer for receiving Lightning payments |
| `bolt12_send` | Pay a BOLT12 Lightning offer |
| `spontaneous_send` | Send a spontaneous (keysend) payment to a Lightning node |
| `unified_send` | Send a payment given a BIP 21 URI or BIP 353 Human-Readable Name |

### Channels
| Tool | Description |
|------|-------------|
| `open_channel` | Open a new Lightning channel with a remote node |
| `close_channel` | Cooperatively close a Lightning channel |
| `force_close_channel` | Force close a Lightning channel unilaterally |
| `list_channels` | List all known Lightning channels |
| `update_channel_config` | Update forwarding fees and CLTV delta for a channel |
| `splice_in` | Increase a channel's balance by splicing in on-chain funds |
| `splice_out` | Decrease a channel's balance by splicing out to on-chain |

### Payment History
| Tool | Description |
|------|-------------|
| `list_payments` | List all payments (supports pagination via page_token) |
| `get_payment_details` | Get details of a specific payment by its ID |
| `list_forwarded_payments` | List all forwarded payments (supports pagination via page_token) |

### Peers
| Tool | Description |
|------|-------------|
| `connect_peer` | Connect to a Lightning peer without opening a channel |
| `disconnect_peer` | Disconnect from a Lightning peer |
| `list_peers` | List all known Lightning peers |

### Utilities
| Tool | Description |
|------|-------------|
| `decode_invoice` | Decode a BOLT11 invoice and return its parsed fields |
| `decode_offer` | Decode a BOLT12 offer and return its parsed fields |
| `sign_message` | Sign a message with the node's secret key |
| `verify_signature` | Verify a signature against a message and public key |
| `export_pathfinding_scores` | Export the pathfinding scores used by the Lightning router |

## MCP Protocol

- **Protocol version**: `2025-11-25`
- **Transport**: stdio (one JSON-RPC 2.0 message per line)
- **Methods**: `initialize`, `tools/list`, `tools/call`

## Testing

```bash
cargo test
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
