# Configuration

LDK Server can be configured via a TOML file, environment variables, or CLI arguments.
The [annotated config template](../contrib/ldk-server-config.toml) shows every available
option with comments and is the canonical reference for individual fields.

## Precedence

When the same option is set in multiple places, the highest-priority source wins:

1. **CLI arguments** (highest)
2. **Environment variables** (`LDK_SERVER_*` prefix)
3. **TOML config file**
4. **Built-in defaults** (lowest)

## CLI Arguments

All CLI flags use long-form hyphenated names derived from the TOML keys. For example,
`node.network` becomes `--node-network`, `bitcoind.rpc_address` becomes
`--bitcoind-rpc-address`, etc. See `ldk-server --help` for the full list of options.

```bash
ldk-server path/to/config.toml --node-network signet
```

## Environment Variables

All environment variables use the `LDK_SERVER_` prefix. For example, `node.network` in the
TOML becomes `LDK_SERVER_NODE_NETWORK`, `bitcoind.rpc_address` becomes
`LDK_SERVER_BITCOIND_RPC_ADDRESS`, etc. See `ldk-server --help` for the full list of options
and their corresponding environment variables.

```bash
LDK_SERVER_NODE_NETWORK=signet ldk-server /path/to/config.toml
```

## Config File

Pass a TOML file as a positional argument:

```bash
ldk-server /path/to/config.toml
```

If no file is provided, the server looks for `config.toml` in the default storage directory
(`~/.ldk-server/config.toml` on Linux, `~/Library/Application Support/ldk-server/config.toml`
on macOS).

## Config Sections

### `[node]`

Core node settings: which Bitcoin network to use, Lightning peer listening and announcement
addresses, the gRPC bind address, node alias, optional Rapid Gossip Sync / pathfinding
scores URLs, the async payments role, and zero-fee commitment channels.

Set `async_payments_role = "client"` to ask peers to hold HTLCs where possible, allowing
this node to go offline. Set `async_payments_role = "server"` to hold async payment HTLCs
and onion messages for peers. The server role requires an announceable node configuration.
Leave the field unset to disable async payments.

Set `enable_zero_fee_commitments = true` to prefer zero-fee commitment channels,
falling back to other channel types supported by the peer. Enabling this setting requires a chain
source that supports Bitcoin Core's `submitpackage` RPC, and relays TRUC, P2A, and ephemeral
dust. The default is `false`.

### `[probing]`

Enables LDK Node's background probing service to train the payment scorer with current
channel-liquidity information. Probing is disabled when this section and the corresponding
CLI/environment options are absent.

The `strategy` field selects one of two path-selection methods:

- **`"high_degree"`** probes toward highly connected public nodes. The service uses the
  payment scorer to select a path.
- **`"random_walk"`** constructs random paths through the public graph. This strategy does
  not use the payment scorer to select a path.

The section accepts these fields:

| Field | Requirement and default | Description |
| --- | --- | --- |
| `strategy` | Required. No default. | Selects `"high_degree"` or `"random_walk"`. Start with `"random_walk"` for a payment node. Use `"high_degree"` for a dedicated probing node. |
| `top_node_count` | Required for `"high_degree"`. No default. Start with `100`. | Sets the number of highly connected public nodes in the destination set. The strategy cycles through this set. The value must be greater than `0`. |
| `max_hops` | Required for `"random_walk"`. No default. Start with `5`. | Sets the maximum number of hops in a random path. The value must be at least `2`. LDK Node changes values greater than `19` to `19`. |
| `interval_secs` | Optional. The default is `10`. | Sets the number of seconds between probe attempts. LDK Node changes `0` to its minimum interval of 100 milliseconds. |
| `max_locked_msat` | Optional. The default is `100000000` (100,000 satoshis). | Limits the total amount and pending fees of in-flight probes. The service skips a probe that exceeds the remaining limit. Built-in probes use 1,000,000 through 10,000,000 millisatoshis before routing fees. |
| `diversity_penalty_msat` | Optional. The default is `0`. | Adds a virtual routing cost to recently probed channels. The service does not pay this amount. The cost decreases to zero over 24 hours and encourages different paths. This field only affects `"high_degree"`. |
| `cooldown_secs` | Optional. The default is `3600`. | Sets the time before `"high_degree"` can select the same destination again. The strategy starts a new cycle immediately after it probes all destinations. |

```toml
[probing]
strategy = "high_degree"
top_node_count = 100
interval_secs = 30
max_locked_msat = 100000000
diversity_penalty_msat = 250
cooldown_secs = 3600
```

> [!CAUTION]
> Probes send real HTLCs over real channels. A probe can lock outbound liquidity
> until its HTLC expires. Use `max_locked_msat` to limit this risk.

### `[storage.disk]`

Where local ldk-server data is stored. Defaults to `~/.ldk-server/` on Linux and
`~/Library/Application Support/ldk-server/` on macOS. This directory is still used for
the node mnemonic, API key, TLS material, logs, and ldk-server's payment history when
LDK Node wallet/channel state uses PostgreSQL.

### `[storage.postgres]`

Optional PostgreSQL storage for LDK Node wallet and channel state.

```toml
[storage.postgres]
connection_string = "postgresql://postgres:postgres@localhost:5432"
db_name = "ldk_db"
kv_table_name = "ldk_data"
certificate_path = "/path/to/postgres-ca.pem"
```

Only `connection_string` is required. `db_name`, `kv_table_name`, and `certificate_path`
are optional. If `db_name` is set, do not also include a database name in the connection
string. If `certificate_path` is set, the file must contain a PEM-encoded CA certificate
for TLS PostgreSQL connections.

Storage migration is not supported. ldk-server refuses to start with PostgreSQL when an existing
`ldk_node_data.sqlite` file is present. After the first successful PostgreSQL node build,
ldk-server creates `<network_dir>/ldk_node_postgres.lock` and refuses to start with SQLite while
that file exists.

### `[log]`

Controls logging behavior. By default, `log_to_file` is `true` and logs are also written 
to `stdout`/`stderr`.

If `log_to_file` is enabled, logs are written to the configured file while still keeping 
the `stdout`/`stderr` logs available too. Logs files are automatically rotated at 
`max_size_mb` or `rotation_interval_hours`. To disable the internal rotation and keep 
logging to file, set `max_size_mb` and `rotation_interval_hours` params to `0`.

The server will also reopen the log file on `SIGHUP` for compatibility with external 
tools like `logrotate`.

### `[tls]`

TLS certificate and key paths, plus additional hostnames/IPs for the certificate's Subject
Alternative Names. If no certificate exists, the server auto-generates a self-signed ECDSA
P-256 cert. `localhost` and `127.0.0.1` are always included in the SANs. Add your server's
public hostname or IP to `hosts` if clients connect remotely.

To bring your own certificate (for example, from a public CA), set `cert_path` and
`key_path`. The server reads these files on startup, so renewals require a restart.
See [Operations - TLS](operations.md#tls) for a recommended CA-signed flow.

### Bitcoin Backend

You must configure **exactly one** of the following sections:

- **`[bitcoind]`** - Bitcoin Core RPC. **Recommended.** Most reliable and private option.
  Required for production deployments.
- **`[electrum]`** - Electrum server. Lighter weight, but relies on a trusted third-party
  server for chain data.
- **`[esplora]`** - Esplora HTTP API. Convenient for quick testing with a public block
  explorer (e.g., mempool.space), but not recommended for production use.

> **Warning:** When using Electrum or Esplora, LDK cannot verify Lightning gossip messages
> against the blockchain. This means a malicious peer could flood your node with fake channel
> announcements, consuming memory and disk. If your node is publicly reachable, use bitcoind.

### `[liquidity.lsps2_client]`

Connects to an [LSPS2](https://github.com/BitcoinAndLightningLayerSpecs/lsp/blob/main/LSPS2/README.md)
Liquidity Service Provider for just-in-time (JIT) inbound channel opening. When configured,
the `Bolt11ReceiveViaJitChannel` and `Bolt11ReceiveVariableAmountViaJitChannel` RPCs become
available, the LSP will open a channel on the fly when the generated invoice is paid.

Requires the LSP's public key and address. Some LSPs also require an authentication token.

### `[liquidity.lsps2_service]`

> Requires building with `--features experimental-lsps2-support`.
> See [Build Features](getting-started.md#build-features).

Configures the node to act as an LSPS2 liquidity service provider, opening JIT channels on
behalf of clients. This involves setting fee parameters (opening fee, minimum fee, overprovisioning
ratio), channel lifetime guarantees, payment size limits, and the trust model.

The `client_trusts_lsp` flag controls when the funding transaction is broadcast: when enabled,
the LSP delays broadcasting until the client has claimed enough HTLC parts to cover the
channel opening cost.

### `[metrics]`

Enables a [Prometheus](https://prometheus.io/) metrics endpoint at `GET /metrics` on the gRPC port, with optional
Basic Auth. See [Operations](operations.md) for scrape configuration.

### `[tor]`

SOCKS proxy address for outbound Tor connections. **Only connections to OnionV3 peers** are
routed through the proxy, other connections (IPv4 peers, Electrum servers, Esplora endpoints)
are not proxied. This does not set up inbound connections, to make your node reachable as a
hidden service, you need to configure Tor separately. See the [Tor guide](tor.md) for the
full setup.

### `[hrn]`

Configures how the node resolves [BIP 353](https://github.com/bitcoin/bips/blob/master/bip-0353.mediawiki)
Human-Readable Names (e.g., `₿alice@example.com`) to Lightning payment destinations.

Two resolution methods are supported via the `mode` field:

- **`"dns"`** (default) - Resolve names locally using a DNS server. The server is set via
  `dns_server_address` (default: `8.8.8.8:53`, Google Public DNS). The port defaults to
  `53` if omitted. When `enable_resolution_service = true`, the node additionally offers
  HRN resolution to the rest of the network over Onion Messages. This requires the node
  to be announceable so resolution requests can be routed to it, and is therefore
  disabled by default.
- **`"blip32"`** - Ask other nodes to resolve names on our behalf via
  [bLIP-32](https://github.com/lightning/blips/blob/master/blip-0032.md). `dns_server_address`
  and `enable_resolution_service` only apply in `"dns"` mode and are rejected here.

## Storage Layout

```
<storage_dir>/
  keys_mnemonic          # BIP39 mnemonic (default for new installs)
  tls.crt                # TLS certificate (PEM)
  tls.key                # TLS private key (PEM)
  <network>/                # e.g., bitcoin/, regtest/, signet/
    api_key                # API key
    ldk-server.log         # Log file
    ldk_node_postgres.lock  # Present when LDK Node state uses PostgreSQL
    ldk_node_data.sqlite   # LDK Node state (channels, on-chain wallet)
    ldk_server_data.sqlite # Payment and forwarding history
```

The mnemonic is the node's master secret, required to recover on-chain funds. On first start,
ldk-server generates a fresh 24-word BIP39 mnemonic at `<storage_dir>/keys_mnemonic` if the file
does not already exist. `ldk_node_data.sqlite` holds channel state, both are required to recover
channel funds. See [Operations - Backups](operations.md#backups) for backup guidance.

When `[storage.postgres]` is configured, LDK Node wallet/channel state is stored in
PostgreSQL instead of `ldk_node_data.sqlite`. The storage directory remains required for
the mnemonic, API key, TLS material, logs, and `ldk_server_data.sqlite`.
