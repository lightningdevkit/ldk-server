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
5. Builds `ldk-server` from a **pinned commit** (the `LDK_SERVER_COMMIT` variable
   in the script — a full 40-char SHA, verified after checkout) with the LSPS2
   feature, using a **pinned Rust toolchain** (SHA-256-verified `rustup-init`),
   `cargo build --locked`, and running the build as a throwaway non-root
   `builder` user.
6. Creates the `ldk-server` user + `/var/lib/ldk-server`, wires the chain backend
   (self-hosted bitcoind **or** remote esplora/Mutinynet), and installs the
   systemd unit + hardening drop-in + logrotate.
7. Renders `/etc/ldk-server/config.toml` (`0640`) and a `0600` EnvironmentFile
   for secrets, then **starts** the node (esplora) or leaves it
   **disabled-and-stopped** (bitcoind, pending sync — deliberately not enabled,
   so a reboot cannot auto-start the node and generate the seed before your
   backup).
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
| `chain_backend` | `bitcoind` (self-hosted, non-pruned) or `esplora` (remote). **Mainnet must use `bitcoind`. Mutinynet must use `esplora`** — its 30 s-block signet needs a custom bitcoind build, so the script refuses `mutinynet` + `bitcoind` (stock Bitcoin Core cannot follow it). |
| `esplora_url` | Esplora endpoint for the esplora backend; blank auto-fills `https://mutinynet.com/api` on Mutinynet. |
| `lsp_alias` | Node alias (≤ 32 chars). |
| `announce_ip` | Public IPv4 to announce; blank auto-detects this Linode's IP. |
| `lsps2_require_token` | Gate the service to known clients. **Required on mainnet.** |
| `lsps2_channel_opening_fee_ppm` / `lsps2_min_channel_opening_fee_msat` / `lsps2_max_payment_size_msat` | LSPS2 economics (sane defaults; `max_payment_size` defaults to `330000000` msat, deliberately below the channel-cap ceiling). |
| `metrics_password` | Masked. Blank disables `/metrics`. No whitespace/control chars, quotes, or backslashes. Stored only in the `0600` EnvironmentFile. |

There is no bitcoind RPC password field: for the bitcoind backend the script
generates one on-box and wires it into `bitcoin.conf` (`rpcauth`) and the
EnvironmentFile. It is machine-to-machine on loopback only; use
`bitcoin-cli -datadir=/var/lib/bitcoind` (cookie auth) for manual RPC.

### Plan / disk guidance

- **Build:** ≥ 8 GB RAM (hard requirement; the fat-LTO link must fit in RAM).
- **Mutinynet / signet + esplora:** any ≥ 8 GB plan; no large disk needed.
- **Mainnet + bitcoind:** a **non-pruned** node needs **~600 GB+** disk — use a
  large plan and/or a Linode Block Storage volume mounted at `/var/lib/bitcoind`.
  Initial block download takes hours-to-days.

## After provisioning — the funds-safety sequence

Read `/root/NEXT_STEPS.txt` on the box. In order:

1. **bitcoind path:** wait for sync, then enable + start the node:
   ```bash
   bitcoin-cli -datadir=/var/lib/bitcoind getblockchaininfo   # initialblockdownload=false
   sudo systemctl enable --now ldk-server                     # generates the seed
   ```
   The service is left **disabled** on purpose: enabling it earlier would let a
   reboot (or a Linode host migration) auto-start the node and generate the
   seed unattended, before you've backed it up.
   (esplora path: already started during provisioning.)
2. **Back up the seed offline, by hand:** `/var/lib/ldk-server/keys_mnemonic`
   (24 words). Do **not** add it to the recurring backup job.
3. **Configure and prove a restore:** edit
   `/opt/ldk-server-ops/backup-ldk-server.sh` (`AGE_RECIPIENT`, `RCLONE_REMOTE`),
   run it, and restore the encrypted blob on a **separate clean host**.

   > **Warning:** backups are periodic snapshots and go stale as soon as channel
   > state advances. Restoring a stale snapshot on a **funded** node and letting
   > it run (or force-close) broadcasts since-revoked commitments — the
   > counterparty can sweep the channels via penalty transactions. After any
   > post-funding restore, do not let the node act before assessing. The
   > snapshot cadence bounds the loss window: run the backup job frequently
   > (`sqlite3 .backup` is cheap); live replication (e.g. VSS) is the long-term
   > answer.
4. **Only then fund:** `sudo ldk-server-cli -c /etc/ldk-server/config.toml onchain-receive`
   (mainnet pilot: ≤ 0.05 BTC).
5. Distribute `require_token` to known clients out-of-band.

### Admin access (gRPC stays local-only)

The gRPC API (`127.0.0.1:3536`, also serving `/metrics`) is never exposed. Reach
it via an SSH tunnel:
```bash
ssh -L 3536:localhost:3536 <ssh_user>@<announce_ip>
sudo ldk-server-cli -c /etc/ldk-server/config.toml get-node-info  # on the box
```
On the box, always pass the server config with `-c`: the CLI's default API-key
and TLS-cert paths point at `~/.ldk-server`, but this deployment keeps them
under `/var/lib/ldk-server` (readable only by the service user), and `-c`
resolves them from the config. Do not loosen permissions on
`/var/lib/ldk-server` or `/etc/ldk-server` instead.

### Hard rules

- Never bind `grpc_service_address` to `0.0.0.0` (that exposes full node control).
- Never run two instances on the same identity (= fund loss).
- Keep bitcoind RPC on loopback (the script does this).

## Upgrading

There is no auto-upgrade. **Back up first**, then rebuild from the pinned
source (git + cargo run as the non-root `builder` user; root only installs):
```bash
cd /opt/ldk-server-src
sudo runuser -u builder -- env HOME=/opt/builder RUSTUP_HOME=/opt/rust CARGO_HOME=/opt/cargo \
  bash -c 'git fetch && git checkout <new-commit> && /opt/cargo/bin/cargo build \
  --release --features experimental-lsps2-support'
sudo install -m0755 target/release/ldk-server target/release/ldk-server-cli /usr/local/bin/
sudo systemctl restart ldk-server
```

## Security notes

- Secrets (bitcoind RPC password, metrics password) live in the `0600`
  `/etc/ldk-server/ldk-server.env`, injected by systemd. `require_token` has no
  env var in this `ldk-server` version, so it sits in the `0640`
  `config.toml` (group-readable by the `ldk-server` user).
- The bitcoind RPC password is generated on-box, so it never transits Linode's
  UDF infrastructure. The `metrics_password` UDF does — rotate it if you treat
  it as highly sensitive.
- The StackScript output log `/var/log/stackscript.log` is chmod `600`; the
  script avoids echoing secrets.

## Caveats / verify before mainnet

- **Mutinynet wiring is not yet empirically verified** — confirm `ldk-server`
  (network `signet`) syncs against `https://mutinynet.com/api` on a throwaway
  deploy before relying on it.
- For mainnet bitcoind, the script **enforces** GPG verification of the Bitcoin
  Core `SHA256SUMS` against builder keys pinned in-script by fingerprint (at
  least 2 valid signatures required, or the deploy aborts). The signet/Mutinynet
  path checks the SHA-256 digest only (no real funds). If you bump
  `BITCOIND_VERSION`, confirm the pinned builders still attest that release in
  [bitcoin-core/guix.sigs](https://github.com/bitcoin-core/guix.sigs).
- Tor onion service is not supported in v1 (clearnet announce only).

See [operations.md](operations.md) for backups, monitoring, TLS, and the broader
production runbook.
