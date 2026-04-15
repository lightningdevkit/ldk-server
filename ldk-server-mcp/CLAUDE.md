# CLAUDE.md — ldk-server-mcp

MCP (Model Context Protocol) server that exposes LDK Server operations as tools for AI agents.

## Build / Test Commands

```bash
cargo fmt --all
cargo check
cargo test
cargo clippy
```

## Architecture

```
src/
  main.rs        — Entry point: arg parsing, config, stdio JSON-RPC loop, method dispatch
  config.rs      — Config loading (TOML + env vars), mirrors ldk-server-cli config
  protocol.rs    — JSON-RPC 2.0 request/response types
  mcp.rs         — MCP protocol types (InitializeResult, ToolDefinition, ToolCallResult)
  tools/
    mod.rs       — ToolRegistry: build_tool_registry(), list_tools(), call_tool()
    schema.rs    — JSON Schema definitions for all tool inputs
    handlers.rs  — Handler functions: JSON args -> ldk-server-client call -> JSON result
```

## MCP Protocol

- **Version**: `2025-11-25`
- **Spec**: https://spec.modelcontextprotocol.io/
- **Transport**: stdio (one JSON-RPC 2.0 message per line)
- **Methods implemented**: `initialize`, `tools/list`, `tools/call`
- **Notifications handled**: `notifications/initialized` (ignored, no response)

## Config

The server reads configuration in this precedence order (highest first):

1. **Environment variables**: `LDK_BASE_URL`, `LDK_API_KEY`, `LDK_TLS_CERT_PATH`
2. **CLI argument**: `--config <path>` pointing to a TOML file
3. **Default paths**: `~/.ldk-server/config.toml`, `~/.ldk-server/tls.crt`, `~/.ldk-server/{network}/api_key`

TOML config format (same as ldk-server-cli):
```toml
[node]
grpc_service_address = "127.0.0.1:3536"
network = "bitcoin"

[tls]
cert_path = "/path/to/tls.crt"
```

## Adding a New Tool

When a new endpoint is added to `ldk-server-client`:

1. Add a JSON schema function in `src/tools/schema.rs` (follow existing pattern)
2. Add a handler function in `src/tools/handlers.rs`
3. Register in `build_tool_registry()` in `src/tools/mod.rs`
4. Update the expected tool surface in `tests/integration.rs`
