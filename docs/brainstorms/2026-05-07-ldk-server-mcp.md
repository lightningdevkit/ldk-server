# Brainstorm: ldk-server-mcp — HTTPS MCP gateway for LDK Server

## Clarified Problem Statement

**Goal:** Add a new `ldk-server-mcp` crate that exposes the LDK Server gRPC API as an
HTTPS MCP server, with a bundled web UI for minting named, scoped auth tokens that
users hand to Claude (Desktop / Code) for chat-driven node management.

**Constraints:**
- New crate only — `ldk-server` daemon untouched in v1
- New deps allowed: `rmcp` (MCP Rust SDK) + `axum` (HTTP server) + `argon2`
- Reuse existing `ldk-server-client` to call the daemon's gRPC over its self-signed TLS
- Reuse the daemon's TLS bootstrap pattern (auto-generate self-signed cert in storage dir)
- Match existing code style: Apache-2.0/MIT dual license header, `cargo fmt`, no surprise abstractions

**Non-goals:**
- Multi-tenant / multi-node management (one MCP per ldk-server)
- OAuth 2.1 / SSO (bearer tokens only for v1)
- Public-internet hosting story (assume same-host or trusted-network deploy)
- Modifying the daemon's existing single-`api_key` HMAC auth in v1

**Success criteria:**
- `cargo run -p ldk-server-mcp -- <config>` boots, serves UI on `https://localhost:<port>/`
- First-run prints a one-shot bootstrap admin token; admin signs in and mints a named
  token with scopes
- "Connect to Claude" UI flow produces a copy-paste MCP server config (Streamable HTTP
  URL + bearer)
- All ~40 LDK Server RPCs are reachable as MCP tools, filtered by token scope
- Adding a tool or scope is a small, mechanical edit

## Approaches Considered

### Approach A — Gateway with its own auth, daemon untouched (Selected)
- **Sketch:** `ldk-server-mcp` holds the sqlite token store. Tokens have a scope set
  (which MCP tools they can call). The gateway holds the daemon's single `api_key`
  and is a fully-trusted client. UI lives at `axum` route `/`, MCP at `/mcp`
  (Streamable HTTP).
- **Affected files/modules:** new `ldk-server-mcp/` crate; `Cargo.toml` workspace
  `members`; `ldk-server-client` as path dep; nothing in `ldk-server/` or
  `ldk-server-grpc/`.
- **Tradeoffs:** Fastest to ship. Scope enforcement is at the gateway layer only —
  a compromised gateway has full daemon access via the embedded api_key.
  Acceptable when gateway and daemon co-locate on a trusted host.
- **Effort:** M.

### Approach B — Token store in the daemon, MCP is a thin proxy
- **Sketch:** Daemon gains a sqlite `auth_tokens` table + bearer-token auth path
  alongside the existing HMAC. Tokens carry scopes; daemon checks scopes per RPC.
  MCP gateway just translates MCP tool calls → gRPC and forwards the bearer.
- **Affected files/modules:** new `ldk-server-mcp/` crate **plus** changes in
  `ldk-server/src/service.rs`, `ldk-server/src/io/persist/`,
  `ldk-server-grpc/src/proto/api.proto`, `ldk-server/src/api/`.
- **Tradeoffs:** Real scope enforcement at the trust boundary; revoking a token
  in the daemon stops it for any client immediately. Touches `api.proto`, which
  means a wire-level review and an upstream PR is bigger.
- **Effort:** L.

### Approach C — Phased: ship A first, migrate to B in v2
- **Sketch:** v1 = Approach A. Token format on disk is designed forward-compatibly
  so v2 can lift it into the daemon's sqlite without re-issuing tokens. v2 adds
  the daemon-side `auth_tokens` table and the MCP gateway becomes a thin proxy.
- **Tradeoffs:** Get something testable in front of Claude in 1–2 PRs. Defers the
  bigger auth refactor until UX is validated.
- **Effort:** M (v1) + L (v2 later).

## Recommendation

**Approach A** for v1 (chosen by user). The user's combination of "full API surface"
+ "named tokens with scopes" eventually wants real daemon-side enforcement (Approach
B), but doing B in one shot means a `.proto` change, new admin RPCs, and a new auth
path before anyone has even held the new feature in their hands. Ship A as v1 to
validate the UX, the rmcp integration, and the Claude config flow; promote token
storage into the daemon as v2 once the shape is settled.

---

## v1 Spec

### Shape
- New workspace crate. Single binary. Talks to the daemon via existing
  `ldk-server-client` over the daemon's gRPC + TLS. Daemon untouched.
- Single `axum` listener with TLS, four surfaces:
  - `/` and `/assets/*` — embedded UI (`include_str!` of the HTML)
  - `/api/*` — UI REST endpoints, HttpOnly-cookie authed
  - `/mcp` — MCP Streamable HTTP endpoint, `Authorization: Bearer <token>` authed
  - `/healthz` — liveness probe (no auth)

### Config (`ldk-server-mcp.toml`)

```toml
[gateway]
listen_addr = "127.0.0.1:3537"
storage_dir = "/var/lib/ldk-server-mcp"
log_level   = "info"

[gateway.tls]
# Optional. Omit to auto-generate self-signed cert in storage_dir.
cert_path = "/etc/ldk-server-mcp/tls.crt"
key_path  = "/etc/ldk-server-mcp/tls.key"

[daemon]
address       = "https://127.0.0.1:3536"
api_key_path  = "/var/lib/ldk-server/bitcoin/api_key"
tls_cert_path = "/var/lib/ldk-server/tls.crt"
```

### Auth model (two token types)
- **Bootstrap admin token** — generated on first run, printed once to stdout, hash
  stored at `<storage_dir>/bootstrap_token.hash` (argon2id). Used only to sign into
  the UI. Rotatable via `ldk-server-mcp rotate-admin-token <config>`.
- **Named tokens** — minted from the UI, written to `<storage_dir>/mcp.sqlite`,
  used by Claude. Bearer format: `lsmcp_<base64url-32B>`. Stored as argon2id hash
  plus an 8-char prefix for UI display.

### sqlite schema (`mcp.sqlite`)

```sql
CREATE TABLE auth_tokens (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  token_id      TEXT    NOT NULL UNIQUE,  -- public id, hex(16B)
  name          TEXT    NOT NULL UNIQUE,  -- [a-z0-9-]+, 1..40
  secret_hash   TEXT    NOT NULL,         -- argon2id
  secret_prefix TEXT    NOT NULL,         -- first 8 chars of bearer
  scopes        TEXT    NOT NULL,         -- JSON array
  created_at    INTEGER NOT NULL,
  revoked_at    INTEGER,
  last_used_at  INTEGER
);
CREATE INDEX idx_auth_tokens_token_id ON auth_tokens(token_id);
```

`last_used_at` updates are debounced — one write per token per 60s, in a background task.

### Scope catalog

| Scope | RPCs |
|---|---|
| `read-only` | `get_node_info`, `get_balances`, `list_channels`, `list_payments`, `get_payment_details`, `list_forwarded_payments`, `list_peers`, `graph_list_channels`, `graph_get_channel`, `graph_list_nodes`, `graph_get_node`, `decode_invoice`, `decode_offer`, `export_pathfinding_scores` |
| `receive` | `bolt11_receive`, `bolt11_receive_for_hash`, `bolt11_claim_for_hash`, `bolt11_fail_for_hash`, `bolt11_receive_via_jit_channel`, `bolt11_receive_variable_amount_via_jit_channel`, `bolt12_receive`, `onchain_receive` |
| `payments` | `bolt11_send`, `bolt12_send`, `onchain_send`, `spontaneous_send`, `unified_send` |
| `channels` | `open_channel`, `close_channel`, `force_close_channel`, `splice_in`, `splice_out`, `update_channel_config` |
| `peers` | `connect_peer`, `disconnect_peer` |
| `signing` | `sign_message`, `verify_signature` |
| `events` | streaming notifications only |
| `admin` | superset of all of the above |

### MCP tool layer
- 1:1 mapping: each daemon RPC → one MCP tool (snake_case names).
- JSON Schema for inputs hand-written for v1 (small per-tool modules).
- Each handler: validate scope → deserialize args → call `LdkServerClient` method
  → serialize response. gRPC errors map to MCP errors via a small status-code table.
- The MCP catalog response (`tools/list`) is filtered per-bearer so Claude only
  sees what it can call.

### SubscribeEvents → MCP notifications
- One long-lived gRPC subscription from the gateway to the daemon, fanned out
  internally with a `tokio::broadcast` channel.
- Each MCP session whose token has `events` (or `admin`) gets forwarded events as
  MCP `notifications/message` with method `ldk-server/event` and the
  `EventEnvelope` JSON as params.

### Crate layout

```
ldk-server-mcp/
  Cargo.toml
  src/
    main.rs                  # clap, config load, axum boot, signal handling
    config.rs                # TOML schema + validation
    storage.rs               # sqlite migrations + token DAO + session DAO
    auth/
      session.rs             # UI cookie sessions
      bearer.rs              # MCP bearer validation + scope check
      bootstrap.rs           # admin token gen/rotate
    daemon/
      client.rs              # wraps ldk-server-client::LdkServerClient
      events.rs              # SubscribeEvents fan-out
    mcp/
      tools.rs               # catalog, scope filtering, dispatch
      handlers/              # node, balances, payments, channels, peers, signing, graph
      notifications.rs       # event -> notification
    web/
      routes.rs              # /api/* axum routes
      static_assets.rs       # include_str!("../ui/index.html")
    util/
      tls.rs                 # copy of daemon's pattern
      logger.rs              # reuse ServerLogger style
  ui/
    index.html               # produced by the UI design prompt
```

### New deps (approved)
- `rmcp` (modelcontextprotocol/rust-sdk) — Streamable HTTP server transport
- `axum`, `tower`, `tower-http` — HTTP, middleware
- `argon2` — token hashing
- `tower-cookies` (or `cookie` directly) — UI session cookie

Already in tree and reused: `tokio`, `tokio-rustls`, `ring`, `rusqlite`, `serde` /
`serde_json`, `prost`, `chrono`, `clap`, `log`, `hex-conservative`, `base64`,
`getrandom`, `ldk-server-client`, `ldk-server-grpc`.

### First-run UX

```
$ ldk-server-mcp /etc/ldk-server-mcp/config.toml
[INFO] Generated bootstrap admin token (save it now):
    lsmcp-admin_4n7q...G2k8
[INFO] gateway listening on https://127.0.0.1:3537
[INFO] daemon connection ok (alias='alice', network='signet')
```

### Suggested PR breakdown

1. **PR1 — crate skeleton** — workspace member, config, TLS bootstrap, axum
   listener, healthz, daemon connection check. No auth, no UI, no MCP yet.
2. **PR2 — sqlite + auth** — token DAO, argon2 hashing, bootstrap admin token,
   UI session cookie, `/api/login`, `/api/tokens` (list/create/revoke).
3. **PR3 — UI** — drop in the HTML, wire up `/api/*` endpoints, basic dashboard.
4. **PR4 — MCP read-only tools** — `rmcp` integration, tool catalog filtered by
   scope, handlers for the read RPCs.
5. **PR5 — MCP write scopes** — handlers for `payments`, `receive`, `channels`,
   `peers`, `signing`.
6. **PR6 — Events** — daemon `SubscribeEvents` fan-out, MCP notifications.
7. **PR7 — Connect-to-Claude UX** — `/api/connect/snippet` + UI tab.
8. **PR8 — operational polish** — `rotate-admin-token` subcommand, structured
   logs, systemd unit example, README and `docs/mcp-gateway.md`.

### Deferred to v2
- Daemon-side `auth_tokens` (Approach B)
- OAuth 2.1 dynamic client registration
- Prometheus metrics on the gateway
- Per-token rate limits
- Multi-node fan-out

## Open Questions (resolved)
- MCP transport: **Streamable HTTP only**
- UI tech: **single self-contained HTML, no build step**
- Bootstrap admin token: **printed once on first run, hash stored on disk**
- Listening port: **127.0.0.1:3537** (default)
- Same binary for UI + MCP: **yes**
