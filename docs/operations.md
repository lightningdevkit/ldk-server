# Operations Guide

This guide covers running LDK Server in production: process management, backups, security,
monitoring, and remote access.

## Process Management

### systemd

LDK Server integrates with systemd via `sd_notify`. It sends `READY=1` after the gRPC listener
is bound and `STOPPING=1` when shutting down. A sample unit file can be found in [
`contrib/ldk-server.service`](../contrib/ldk-server.service).

### Graceful Shutdown

The server handles `SIGTERM` and `CTRL-C` (SIGINT). On receipt, it:

1. Signals all active streaming clients (SubscribeEvents) to disconnect
2. Stops the LDK Node (persists channel state)
3. Exits cleanly

### Log Rotation

> **Important:** LDK Server does not rotate or truncate its own log file. Without log rotation
> configured, the log file will grow indefinitely and can eventually fill your disk. A full
> disk can prevent the node from persisting channel state, risking fund loss.

The server reopens its log file on `SIGHUP`. This integrates with standard `logrotate`. Save
the following config to `/etc/logrotate.d/ldk-server` (adjust the log path to match your
setup):

```
/var/lib/ldk-server/regtest/ldk-server.log {
    daily
    rotate 14
    compress
    missingok
    notifempty
    postrotate
        systemctl kill --signal=HUP ldk-server.service
    endscript
}
```

## Backups

### What to Back Up

| File                                   | Priority     | Description                                                                |
|----------------------------------------|--------------|----------------------------------------------------------------------------|
| `<storage_dir>/keys_mnemonic`          | **Critical** | BIP39 mnemonic. Required to recover on-chain funds. Default for new installs. |
| `<storage_dir>/keys_seed`              | **Critical** | Legacy raw seed file. Only present on installs initialized before mnemonic support. |
| `<network_dir>/ldk_node_data.sqlite`   | **Critical** | Channel state and on-chain wallet data. Required to recover channel funds. |
| `<network_dir>/ldk_server_data.sqlite` | Nice-to-have | Payment and forwarding history                                             |

### What is Reconstructable

- Network graph data (re-synced from gossip or RGS)
- Fee rate cache (re-fetched from the chain backend)
- The API key (can be regenerated, but clients will need the new one)
- The TLS certificate (can be regenerated, but clients will need the new one)

> **Warning:** Do not restore a backup onto two running nodes simultaneously. Running the
> same node identity on two instances will cause channel state conflicts and potential fund
> loss.

## Security

### API Key

- Auto-generated as 32 random bytes on first startup
- Stored at `<network_dir>/api_key` with `0400` permissions (read-only for owner)
- The hex-encoded form of this key is used for HMAC authentication
- Treat it as a secret: anyone with the API key and network access to the gRPC port can
  control the node

### TLS

- Self-signed ECDSA P-256 certificate generated automatically
- Private key stored at `<storage_dir>/tls.key` with `0400` permissions
- Certificate includes `localhost` and `127.0.0.1` in SANs by default
- Add your server's hostname/IP to `[tls] hosts` for remote access

### Network Exposure

The gRPC service binds to `127.0.0.1:3536` by default. For remote access, either:

1. Change `grpc_service_address` to bind to `0.0.0.0:3536` and add the server's hostname to
   `[tls] hosts`, or
2. Use a reverse proxy (e.g., nginx, Caddy) that terminates TLS and forwards to the loopback
   address

## Monitoring

### Prometheus Metrics

LDK Server can expose metrics in [Prometheus](https://prometheus.io/) text format.
Prometheus is an open-source monitoring toolkit that scrapes HTTP endpoints and stores
time-series data for alerting and dashboards.

Enable metrics in the config:

```toml
[metrics]
enabled = true
poll_metrics_interval = 60
```

Metrics are served at `GET /metrics` on the same port as the gRPC service (default 3536).
This is a plain HTTP endpoint (not gRPC), returning Prometheus text format.

Basic Auth is recommended to prevent unauthorized access to node metrics:

```toml
[metrics]
enabled = true
username = "prometheus"
password = "secret"
```

The Prometheus scrape config would then use:

```yaml
scrape_configs:
  - job_name: ldk-server
    scheme: https
    tls_config:
      ca_file: /path/to/tls.crt
    basic_auth:
      username: prometheus
      password: secret
    static_configs:
      - targets: [ 'localhost:3536' ]
```

### Available Metrics

Metrics cover:

- On-chain and Lightning balances
- Public and Private Channel counts
- Payment counts (successful, failed, pending)
- Peer count

## Remote Access

To allow clients to connect from other machines:

1. **Update TLS hosts:** Add the server's hostname or IP to `[tls] hosts` in the config so
   the certificate's SANs cover the address clients will use.
2. **Update bind address:** Set `grpc_service_address` to bind on the appropriate interface
   (e.g., `0.0.0.0:3536`).
3. **Distribute the TLS certificate:** Copy `<storage_dir>/tls.crt` to each client machine.
   Clients must pin this certificate since it is self-signed.
4. **Share the API key:** Provide the hex-encoded API key to authorized clients.

If you regenerate the TLS certificate (by deleting `tls.crt` and `tls.key` and restarting),
all clients will need the new certificate.

## Network-Specific Notes

Data is stored in per-network subdirectories (`bitcoin/`, `testnet/`, `signet/`, `regtest/`,
etc.) under the storage root. This means you can run multiple networks from one storage
directory without conflicts.

The `keys_mnemonic` file (or, on legacy installs, `keys_seed`) is shared across networks
(stored at the storage root, not per-network). Keys are split by network at the derivation
path level, so the same mnemonic/seed will produce different keys.
