# Deploying ldk-server on Linode (StackScript)

[`contrib/linode-stackscript.sh`](../contrib/linode-stackscript.sh) is a
[Linode StackScript](https://techdocs.akamai.com/cloud-computing/docs/automate-deployment-with-stackscripts)
that provisions a fresh Debian/Ubuntu Linode and stands up `ldk-server`, built
from source, as an experimental **LSPS2 Lightning Service Provider**.

> ⚠️ **Experimental + funds-bearing.** This deploys the experimental LSPS2
> service (`experimental-lsps2-support`). On mainnet it custodies real funds.
> **No sats may touch the node until its mnemonic is backed up offline and a
> restore is proven** — both are manual steps the script intentionally does not
> perform. The script always ends with the node **unfunded**.

## What it does

1. Validates inputs and the host (requires **≥ 8 GB RAM** for the fat-LTO build;
   refuses `mainnet` + remote esplora).
2. Adds a 4 GB swap safety margin and installs the toolchain + ops packages.
3. Creates a non-root sudo user, installs your SSH key, hardens `sshd`
   (key-only, no root), and configures UFW (only `22` and `9735/tcp` inbound).
4. Enables unattended security upgrades (no auto-reboot) and fail2ban.
5. Builds `ldk-server` from a **pinned commit** (`c8424db`) with the LSPS2 feature.
6. Creates the `ldk-server` user + `/var/lib/ldk-server`, wires the chain backend
   (self-hosted bitcoind **or** remote esplora/Mutinynet), and installs the
   systemd unit + hardening drop-in + logrotate.
7. Renders `/etc/ldk-server/config.toml` (`0640`) and a `0600` EnvironmentFile
   for secrets, then **starts** the node (esplora) or leaves it
   **enabled-but-stopped** (bitcoind, pending sync).
8. Writes `/root/NEXT_STEPS.txt` and an MOTD with the manual funds-safety sequence.

## Deploy

1. Cloud Manager → **StackScripts → Create** (or use an existing one); paste the
   contents of `contrib/linode-stackscript.sh`. Mark it compatible with recent
   Debian/Ubuntu images.
2. **Create Linode from StackScript**, pick a **≥ 8 GB RAM** plan (mainnet
   bitcoind also needs a large disk — see below), choose a Debian/Ubuntu image,
   and fill the form.
3. Deploy. Provisioning (especially the LTO build) takes a while; follow progress
   in `/var/log/stackscript.log` once you can SSH in.

### UDF fields

| Field | Notes |
|-------|-------|
| `ssh_user` | Non-root sudo admin user (default `lsp`). |
| `ssh_pubkey` | **Required.** Your OpenSSH public key (Ed25519 recommended). A bad key locks you out — recover via the Linode **Lish** console. |
| `ssh_allowed_ips` | Optional CIDR allow-list for SSH; blank = any. |
| `network` | `mainnet` or `mutinynet` (a custom 30 s-block signet). |
| `chain_backend` | `bitcoind` (self-hosted, non-pruned) or `esplora` (remote). **Mainnet must use `bitcoind`.** |
| `esplora_url` | Esplora endpoint for the esplora backend; blank auto-fills `https://mutinynet.com/api` on Mutinynet. |
| `lsp_alias` | Node alias (≤ 32 chars). |
| `announce_ip` | Public IPv4 to announce; blank auto-detects this Linode's IP. |
| `lsps2_require_token` | Gate the service to known clients. **Required on mainnet.** |
| `lsps2_channel_opening_fee_ppm` / `lsps2_min_channel_opening_fee_msat` / `lsps2_max_payment_size_msat` | LSPS2 economics (sane defaults; `max_payment_size` defaults to `330000000` msat, deliberately below the channel-cap ceiling). |
| `bitcoind_rpc_password` | Masked. Required for the bitcoind backend. Stored only in the `0600` EnvironmentFile. |
| `metrics_password` | Masked. Blank disables `/metrics`. Stored only in the EnvironmentFile. |

### Plan / disk guidance

- **Build:** ≥ 8 GB RAM (hard requirement; the fat-LTO link must fit in RAM).
- **Mutinynet / signet + esplora:** any ≥ 8 GB plan; no large disk needed.
- **Mainnet + bitcoind:** a **non-pruned** node needs **~600 GB+** disk — use a
  large plan and/or a Linode Block Storage volume mounted at `/var/lib/bitcoind`.
  Initial block download takes hours-to-days.

## After provisioning — the funds-safety sequence

Read `/root/NEXT_STEPS.txt` on the box. In order:

1. **bitcoind path:** wait for sync, then start the node:
   ```bash
   bitcoin-cli -datadir=/var/lib/bitcoind getblockchaininfo   # initialblockdownload=false
   sudo systemctl start ldk-server                            # generates the seed
   ```
   (esplora path: already started during provisioning.)
2. **Back up the seed offline, by hand:** `/var/lib/ldk-server/keys_mnemonic`
   (24 words). Do **not** add it to the recurring backup job.
3. **Configure and prove a restore:** edit
   `/opt/ldk-server-ops/backup-ldk-server.sh` (`AGE_RECIPIENT`, `RCLONE_REMOTE`),
   run it, and restore the encrypted blob on a **separate clean host**.
4. **Only then fund:** `ldk-server-cli onchain-receive` (mainnet pilot: ≤ 0.05 BTC).
5. Distribute `require_token` to known clients out-of-band.

### Admin access (gRPC stays local-only)

The gRPC API (`127.0.0.1:3536`, also serving `/metrics`) is never exposed. Reach
it via an SSH tunnel:
```bash
ssh -L 3536:localhost:3536 <ssh_user>@<announce_ip>
ldk-server-cli get-node-info
```

### Hard rules

- Never bind `grpc_service_address` to `0.0.0.0` (that exposes full node control).
- Never run two instances on the same identity (= fund loss).
- Keep bitcoind RPC on loopback (the script does this).

## Upgrading

There is no auto-upgrade. **Back up first**, then rebuild from the pinned source:
```bash
cd /opt/ldk-server-src && sudo git fetch && sudo git checkout <new-commit>
sudo env CARGO_HOME=/opt/cargo RUSTUP_HOME=/opt/rust /opt/cargo/bin/cargo build \
  --release --features experimental-lsps2-support
sudo install -m0755 target/release/ldk-server target/release/ldk-server-cli /usr/local/bin/
sudo systemctl restart ldk-server
```

## Security notes

- Secrets (bitcoind RPC password, metrics password) live in the `0600`
  `/etc/ldk-server/ldk-server.env`, injected by systemd. `require_token` has no
  env var in this `ldk-server` version, so it sits in the `0640`
  `config.toml` (group-readable by the `ldk-server` user).
- The StackScript output log `/var/log/stackscript.log` is chmod `600`; the
  script avoids echoing secrets. UDF passwords still transit Linode's
  infrastructure — rotate them if you treat them as highly sensitive.

## Caveats / verify before mainnet

- **Mutinynet wiring is not yet empirically verified** — confirm `ldk-server`
  (network `signet`) syncs against `https://mutinynet.com/api` on a throwaway
  deploy before relying on it.
- For mainnet bitcoind, additionally **GPG-verify** the Bitcoin Core `SHA256SUMS`
  against the builder keys (the script only checks the SHA-256 digest).
- Tor onion service is not supported in v1 (clearnet announce only).

See [operations.md](operations.md) for backups, monitoring, TLS, and the broader
production runbook.
