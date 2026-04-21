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
addresses, the gRPC bind address, node alias, and optional Rapid Gossip Sync / pathfinding
scores URLs.

### `[storage.disk]`

Where persistent data is stored. Defaults to `~/.ldk-server/` on Linux and
`~/Library/Application Support/ldk-server/` on macOS.

### `[log]`

Log level and file path. The server reopens the log file on `SIGHUP`, which integrates with
standard `logrotate` setups.

### `[tls]`

TLS certificate and key paths, plus additional hostnames/IPs for the certificate's Subject
Alternative Names. If no certificate exists, the server auto-generates a self-signed ECDSA
P-256 cert. `localhost` and `127.0.0.1` are always included in the SANs. Add your server's
public hostname or IP to `hosts` if clients connect remotely.

To bring your own certificate (e.g., from Let's Encrypt), set `cert_path` and `key_path`.

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
  keys_seed              # Node entropy/seed
  tls.crt                # TLS certificate (PEM)
  tls.key                # TLS private key (PEM)
  <network>/                # e.g., bitcoin/, regtest/, signet/
    api_key                # API key
    ldk-server.log         # Log file
    ldk_node_data.sqlite   # LDK Node state (channels, on-chain wallet)
    ldk_server_data.sqlite # Payment and forwarding history
```

The `keys_seed` file is the node's master secret, required to recover on-chain funds.
`ldk_node_data.sqlite` holds channel state, both are required to recover channel funds. See
[Operations - Backups](operations.md#backups) for backup guidance.
