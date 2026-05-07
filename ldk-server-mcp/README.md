# ldk-server-mcp

`ldk-server-mcp` is an HTTPS gateway that sits in front of an LDK Server daemon and (eventually)
exposes its API as an MCP (Model Context Protocol) server, plus a small admin web UI for minting
named, scoped auth tokens to plug into Claude Desktop / Claude Code.

## Status

This crate is the **v1 scaffolding only**. It currently:

- Loads a TOML config (see `contrib/ldk-server-mcp-config.toml`)
- Auto-generates a self-signed TLS certificate in the storage directory (or uses paths from
  the config), mirroring the daemon's pattern
- Verifies it can reach the upstream daemon on boot by calling `GetNodeInfo`
- Serves a single `/healthz` endpoint over HTTPS via `axum`
- Handles `SIGTERM`, `SIGHUP` (log reopen), and `Ctrl-C`

The token store, UI, MCP tool layer, and event-streaming notifications are landing in
follow-up PRs. See `docs/brainstorms/2026-05-07-ldk-server-mcp.md` for the full v1 spec and PR
breakdown.

## Build & run

```bash
cargo build --release -p ldk-server-mcp
cp contrib/ldk-server-mcp-config.toml my-mcp.toml  # edit paths to match your daemon
./target/release/ldk-server-mcp my-mcp.toml
```

Probe the health endpoint (use `-k` because the cert is self-signed):

```bash
curl -k https://127.0.0.1:3537/healthz
# -> ok
```

## Architecture

`ldk-server-mcp` is a separate workspace crate from the `ldk-server` daemon. It calls the daemon
over its existing gRPC + TLS interface using `ldk-server-client`. The daemon is unmodified.
