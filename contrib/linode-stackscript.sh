#!/bin/bash
# ============================================================================
# ldk-server — Linode StackScript (LSPS2 Lightning Service Provider)
#
# Deploys ldk-server, built from source, as an experimental LSPS2 LSP on a
# fresh Debian/Ubuntu Linode. Hardens the host, builds the binary, writes a
# systemd service, configures the firewall, and STOPS at the funds-safety
# boundary — the node ends UNFUNDED and the seed is never printed.
#
# Modeled on Blockchain Commons' LinodeStandUp.sh. See docs/linode-stackscript.md.
#
# ⚠️  This deploys the EXPERIMENTAL LSPS2 service. On mainnet it custodies real
#     funds. NO sats may touch the node until its mnemonic is backed up offline
#     AND a restore is proven — both are manual steps this script cannot do.
#
# Plan:      docs/plans/2026-06-30-001-feat-linode-stackscript-ldk-server-lsp-plan.md
# ============================================================================

# --- UDF (deploy-time form fields; required unless a default is given) ------
# <UDF name="ssh_user" label="Admin username (non-root sudo)" default="lsp" />
# <UDF name="ssh_pubkey" label="Admin SSH public key (Ed25519 recommended)" />
# <UDF name="ssh_allowed_ips" label="Restrict SSH to these IPs (CIDR, comma-separated; blank = any)" default="" />
# <UDF name="network" label="Bitcoin network" oneOf="mainnet,mutinynet" default="mutinynet" />
# <UDF name="chain_backend" label="Chain backend (mainnet must use bitcoind)" oneOf="bitcoind,esplora" default="esplora" />
# <UDF name="esplora_url" label="Esplora URL (esplora backend only; blank = auto for Mutinynet)" default="" />
# <UDF name="lsp_alias" label="LSP node alias (<=32 chars)" default="ldk-lsp" />
# <UDF name="announce_ip" label="Announcement IPv4 (blank = auto-detect this Linode's public IP)" default="" />
# <UDF name="lsps2_require_token" label="LSPS2 require_token (gate to known clients; blank = open)" default="" />
# <UDF name="lsps2_channel_opening_fee_ppm" label="LSPS2 opening fee (ppm)" default="1000" />
# <UDF name="lsps2_min_channel_opening_fee_msat" label="LSPS2 min opening fee (msat)" default="10000000" />
# <UDF name="lsps2_max_payment_size_msat" label="LSPS2 max payment size (msat)" default="330000000" />
# <UDF name="bitcoind_rpc_password" label="bitcoind RPC password (bitcoind backend only; masked)" default="" />
# <UDF name="metrics_password" label="Prometheus /metrics Basic-Auth password (masked; blank = metrics off)" default="" />

# --- Strict mode + logging --------------------------------------------------
set -euo pipefail
# StackScript stdout is NOT streamed; persist it. UDF values arrive as env vars
# and this log would otherwise be world-readable, so lock it down immediately.
LOGFILE=/var/log/stackscript.log
umask 077
touch "$LOGFILE"; chmod 600 "$LOGFILE"
exec > >(tee -ai "$LOGFILE") 2>&1

# Pinned ldk-server commit (no release tags exist; version is 0.1.0).
LDK_SERVER_REPO="https://github.com/lightningdevkit/ldk-server.git"
LDK_SERVER_COMMIT="c8424db"
# bitcoind release used for the self-hosted backend (operator may bump).
BITCOIND_VERSION="29.0"
# Mutinynet (custom signet) parameters — see https://blog.mutinywallet.com/mutinynet/
MUTINYNET_ESPLORA="https://mutinynet.com/api"
MUTINYNET_SIGNETCHALLENGE="512102f7561d208dd9ae99bf497273e16f389bdbd6c4742ddb8e6b216e64fa2928ad8f51ae"
MUTINYNET_ADDNODE="45.79.52.207:38333"

MIN_RAM_MB=7800   # require ~8GB; fat-LTO link must fit in RAM (OQ6)

# --- Helpers ----------------------------------------------------------------
log()  { echo "[stackscript $(date -u +%H:%M:%S)] $*"; }
die()  { echo "[stackscript FATAL] $*" >&2; echo "FAILED: $*" > /root/STACKSCRIPT_FAILED.txt; exit 1; }

# Accept either lower- or upper-case env var for a UDF (Linode injects the name
# verbatim; be defensive about case). Usage: v=$(udf network mainnet)
udf() { local lc="$1" def="${2-}"; local uc; uc=$(echo "$lc" | tr '[:lower:]' '[:upper:]')
        local val="${!lc:-${!uc:-$def}}"; printf '%s' "$val"; }

# --- Read UDF values --------------------------------------------------------
SSH_USER=$(udf ssh_user lsp)
SSH_PUBKEY=$(udf ssh_pubkey "")
SSH_ALLOWED_IPS=$(udf ssh_allowed_ips "")
NETWORK_UDF=$(udf network mutinynet)
CHAIN_BACKEND=$(udf chain_backend esplora)
ESPLORA_URL=$(udf esplora_url "")
LSP_ALIAS=$(udf lsp_alias ldk-lsp)
ANNOUNCE_IP=$(udf announce_ip "")
LSPS2_REQUIRE_TOKEN=$(udf lsps2_require_token "")
LSPS2_FEE_PPM=$(udf lsps2_channel_opening_fee_ppm 1000)
LSPS2_MIN_FEE_MSAT=$(udf lsps2_min_channel_opening_fee_msat 10000000)
LSPS2_MAX_PAYMENT_MSAT=$(udf lsps2_max_payment_size_msat 330000000)
BITCOIND_RPC_PASSWORD=$(udf bitcoind_rpc_password "")
METRICS_PASSWORD=$(udf metrics_password "")

# Map the friendly network name to ldk-server's Network enum value.
case "$NETWORK_UDF" in
	mainnet)  LDK_NETWORK="bitcoin"; NETDIR="bitcoin" ;;
	mutinynet) LDK_NETWORK="signet"; NETDIR="signet" ;;
	*) die "Unknown network '$NETWORK_UDF' (expected mainnet or mutinynet)." ;;
esac

# ============================================================================
# Phase 1 — Validate inputs, plan, and host
# ============================================================================
log "Validating inputs"

[ "$(id -u)" -eq 0 ] || die "StackScript must run as root."
command -v apt-get >/dev/null 2>&1 || die "Unsupported image: apt-get not found (use Debian/Ubuntu)."

# SSH key sanity — a bad/empty key locks the operator out of the new user.
case "$SSH_PUBKEY" in
	ssh-ed25519\ *|ssh-rsa\ *|ecdsa-sha2-*\ *|sk-ssh-ed25519@openssh.com\ *) : ;;
	*) die "ssh_pubkey is empty or not a recognized OpenSSH public key." ;;
esac

# Numeric LSPS2 fields.
for v in "$LSPS2_FEE_PPM" "$LSPS2_MIN_FEE_MSAT" "$LSPS2_MAX_PAYMENT_MSAT"; do
	[[ "$v" =~ ^[0-9]+$ ]] || die "LSPS2 numeric field has a non-numeric value: '$v'."
done

# Hard-block mainnet + remote chain backend: gossip UTXO verification is
# disabled for esplora/electrum (as_utxo_source() -> None), so a mainnet LSP
# must run its own non-pruned bitcoind. (OQ1)
if [ "$LDK_NETWORK" = "bitcoin" ] && [ "$CHAIN_BACKEND" != "bitcoind" ]; then
	die "mainnet requires chain_backend=bitcoind (remote esplora can't verify gossip UTXOs). Refusing."
fi

# Mainnet LSP must gate the pilot — refuse an open service on real funds.
if [ "$LDK_NETWORK" = "bitcoin" ] && [ -z "$LSPS2_REQUIRE_TOKEN" ]; then
	die "mainnet deployment requires a non-empty lsps2_require_token (gate the pilot to known clients)."
fi

# bitcoind backend needs an RPC password.
if [ "$CHAIN_BACKEND" = "bitcoind" ] && [ -z "$BITCOIND_RPC_PASSWORD" ]; then
	die "chain_backend=bitcoind requires a bitcoind_rpc_password."
fi

# RAM floor for the fat-LTO build (OQ6).
RAM_MB=$(awk '/MemTotal/ {printf "%d", $2/1024}' /proc/meminfo)
log "Detected ${RAM_MB} MB RAM"
[ "${RAM_MB:-0}" -ge "$MIN_RAM_MB" ] || die "Need >= 8 GB RAM to build (have ${RAM_MB} MB). Pick a larger Linode plan."

# Resolve the announcement IP (auto-detect this Linode's public IPv4 if blank).
if [ -z "$ANNOUNCE_IP" ]; then
	ANNOUNCE_IP=$(ip -4 -o addr show scope global 2>/dev/null | awk '{print $4}' | cut -d/ -f1 | head -1)
	[ -n "$ANNOUNCE_IP" ] || ANNOUNCE_IP=$(curl -fsS --max-time 10 https://api.ipify.org 2>/dev/null || true)
fi
[ -n "$ANNOUNCE_IP" ] || die "Could not determine a public IPv4 for announcement_addresses; set the announce_ip UDF."
log "Announcement address: ${ANNOUNCE_IP}:9735"

export DEBIAN_FRONTEND=noninteractive

# ============================================================================
# Phase 2 — Swap, packages, hardening
# ============================================================================
log "Provisioning 4 GB swap safety margin"
if ! swapon --show=NAME --noheadings | grep -q '/swapfile'; then
	fallocate -l 4G /swapfile 2>/dev/null || dd if=/dev/zero of=/swapfile bs=1M count=4096 status=none
	chmod 600 /swapfile
	mkswap /swapfile >/dev/null
	swapon /swapfile
	grep -q '^/swapfile ' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
	printf 'vm.swappiness=10\n' > /etc/sysctl.d/99-ldk-swap.conf
	sysctl --system >/dev/null 2>&1 || true
fi

log "Installing packages"
apt-get update -y
apt-get install -y --no-install-recommends \
	ca-certificates curl git build-essential pkg-config sudo openssl \
	ufw fail2ban unattended-upgrades \
	sqlite3 age rclone jq xxd

log "Creating admin user '${SSH_USER}'"
if ! id -u "$SSH_USER" >/dev/null 2>&1; then
	adduser --disabled-password --gecos "" "$SSH_USER"
fi
usermod -aG sudo "$SSH_USER"
install -d -m 700 -o "$SSH_USER" -g "$SSH_USER" "/home/$SSH_USER/.ssh"
printf '%s\n' "$SSH_PUBKEY" > "/home/$SSH_USER/.ssh/authorized_keys"
chmod 600 "/home/$SSH_USER/.ssh/authorized_keys"
chown "$SSH_USER:$SSH_USER" "/home/$SSH_USER/.ssh/authorized_keys"

log "Hardening sshd (key-only, no root)"
SSH_DROPIN=/etc/ssh/sshd_config.d/99-ldk-hardening.conf
{
	echo "PermitRootLogin no"
	echo "PasswordAuthentication no"
	echo "KbdInteractiveAuthentication no"
	echo "PubkeyAuthentication yes"
	echo "MaxAuthTries 3"
	echo "AllowUsers $SSH_USER"
} > "$SSH_DROPIN"
# Validate before reloading — a broken config would lock everyone out.
sshd -t || die "sshd config validation failed; not reloading. Recover via Linode Lish console."
systemctl reload ssh 2>/dev/null || systemctl reload sshd 2>/dev/null || true

log "Configuring UFW (SSH + 9735 inbound only)"
ufw --force reset >/dev/null
ufw default deny incoming >/dev/null
ufw default allow outgoing >/dev/null
if [ -n "$SSH_ALLOWED_IPS" ]; then
	IFS=',' read -ra _ips <<< "$SSH_ALLOWED_IPS"
	for ip in "${_ips[@]}"; do ip=$(echo "$ip" | xargs); [ -n "$ip" ] && ufw allow from "$ip" to any port 22 proto tcp >/dev/null; done
else
	ufw allow 22/tcp >/dev/null
fi
ufw allow 9735/tcp comment 'Lightning P2P' >/dev/null
ufw --force enable >/dev/null

log "Enabling unattended security upgrades (no auto-reboot) + fail2ban"
printf 'APT::Periodic::Update-Package-Lists "1";\nAPT::Periodic::Unattended-Upgrade "1";\n' \
	> /etc/apt/apt.conf.d/20auto-upgrades
printf 'Unattended-Upgrade::Automatic-Reboot "false";\n' \
	> /etc/apt/apt.conf.d/51ldk-no-reboot
systemctl enable --now fail2ban >/dev/null 2>&1 || true

# ============================================================================
# Phase 3 — Build ldk-server from source (pinned commit)
# ============================================================================
log "Installing Rust toolchain"
export RUSTUP_HOME=/opt/rust CARGO_HOME=/opt/cargo
curl --proto '=https' --tlsv1.2 -fsS https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain stable
export PATH="$CARGO_HOME/bin:$PATH"

log "Cloning ldk-server @ ${LDK_SERVER_COMMIT} and building (this takes a while)"
SRC=/opt/ldk-server-src
rm -rf "$SRC"
git clone "$LDK_SERVER_REPO" "$SRC"
git -C "$SRC" checkout "$LDK_SERVER_COMMIT"
( cd "$SRC" && cargo build --release --features experimental-lsps2-support )

install -m 0755 "$SRC/target/release/ldk-server" /usr/local/bin/ldk-server
install -m 0755 "$SRC/target/release/ldk-server-cli" /usr/local/bin/ldk-server-cli
git -C "$SRC" rev-parse HEAD > /etc/ldk-server-build-commit 2>/dev/null || true
log "Built commit: $(cat /etc/ldk-server-build-commit 2>/dev/null || echo unknown)"

# ============================================================================
# Phase 4 — Service user, directories, chain backend
# ============================================================================
log "Creating ldk-server user and storage dir"
id -u ldk-server >/dev/null 2>&1 || useradd --system --home /var/lib/ldk-server --shell /usr/sbin/nologin ldk-server
install -d -m 700 -o ldk-server -g ldk-server /var/lib/ldk-server
install -d -m 750 -o root -g ldk-server /etc/ldk-server

CHAIN_TOML=""
if [ "$CHAIN_BACKEND" = "bitcoind" ]; then
	log "Setting up self-hosted bitcoind ${BITCOIND_VERSION} (${LDK_NETWORK})"
	id -u bitcoin >/dev/null 2>&1 || useradd --system --home /var/lib/bitcoind --shell /usr/sbin/nologin bitcoin
	install -d -m 710 -o bitcoin -g bitcoin /var/lib/bitcoind
	install -d -m 750 -o root -g bitcoin /etc/bitcoin

	# Download + SHA256-verify Bitcoin Core. NOTE: for mainnet you should also
	# GPG-verify SHA256SUMS against the Bitcoin Core builder keys (see docs).
	arch=$(uname -m); case "$arch" in x86_64) btarch=x86_64-linux-gnu ;; aarch64) btarch=aarch64-linux-gnu ;; *) die "Unsupported arch $arch for bitcoind." ;; esac
	tmp=$(mktemp -d); ( cd "$tmp"
		base="https://bitcoincore.org/bin/bitcoin-core-${BITCOIND_VERSION}"
		curl -fsSLO "${base}/bitcoin-${BITCOIND_VERSION}-${btarch}.tar.gz"
		curl -fsSLO "${base}/SHA256SUMS"
		grep " bitcoin-${BITCOIND_VERSION}-${btarch}.tar.gz\$" SHA256SUMS | sha256sum -c - || exit 1
		tar -xzf "bitcoin-${BITCOIND_VERSION}-${btarch}.tar.gz"
		install -m 0755 "bitcoin-${BITCOIND_VERSION}/bin/bitcoind" "bitcoin-${BITCOIND_VERSION}/bin/bitcoin-cli" /usr/local/bin/
	) || die "bitcoind download/verify/install failed."
	rm -rf "$tmp"

	# rpcauth (salted) — ldk-server has no cookie-auth path, only user/password.
	RPC_USER="ldkserver"
	rpcsalt=$(head -c16 /dev/urandom | xxd -p)
	rpchmac=$(printf '%s' "$BITCOIND_RPC_PASSWORD" | openssl dgst -sha256 -hmac "$rpcsalt" | awk '{print $NF}')

	{
		echo "# Managed by ldk-server linode-stackscript. Loopback RPC only."
		if [ "$LDK_NETWORK" = "signet" ]; then
			echo "signet=1"
			echo "[signet]"
			echo "signetchallenge=${MUTINYNET_SIGNETCHALLENGE}"
			echo "signetblocktime=30"
			echo "addnode=${MUTINYNET_ADDNODE}"
			echo "dnsseed=0"
		else
			echo "chain=main"
		fi
		echo "server=1"
		echo "daemon=0"
		echo "txindex=0"
		echo "dbcache=2048"
		echo "rpcbind=127.0.0.1"
		echo "rpcallowip=127.0.0.1"
		echo "rpcauth=${RPC_USER}:${rpcsalt}\$${rpchmac}"
	} > /etc/bitcoin/bitcoin.conf
	chown root:bitcoin /etc/bitcoin/bitcoin.conf; chmod 640 /etc/bitcoin/bitcoin.conf

	cat > /etc/systemd/system/bitcoind.service <<'EOF'
[Unit]
Description=Bitcoin daemon
After=network-online.target
Wants=network-online.target
[Service]
ExecStart=/usr/local/bin/bitcoind -conf=/etc/bitcoin/bitcoin.conf -datadir=/var/lib/bitcoind
User=bitcoin
Type=simple
Restart=on-failure
TimeoutStartSec=infinity
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=full
ReadWritePaths=/var/lib/bitcoind
[Install]
WantedBy=multi-user.target
EOF
	# RPC is loopback-bound and the default UFW policy denies all inbound, so the
	# RPC port (8332 mainnet / 38332 signet) is never exposed.
	systemctl daemon-reload
	systemctl enable --now bitcoind
	# RPC port: mainnet 8332, signet 38332.
	if [ "$LDK_NETWORK" = "signet" ]; then RPC_PORT=38332; else RPC_PORT=8332; fi
	CHAIN_TOML=$(printf '[bitcoind]\nrpc_address = "127.0.0.1:%s"\nrpc_user = "%s"\n# rpc_password supplied via EnvironmentFile (LDK_SERVER_BITCOIND_RPC_PASSWORD)\n' "$RPC_PORT" "$RPC_USER")
else
	# Remote esplora. Default to the Mutinynet endpoint on signet if unset.
	if [ -z "$ESPLORA_URL" ]; then
		if [ "$LDK_NETWORK" = "signet" ]; then
			ESPLORA_URL="$MUTINYNET_ESPLORA"
		else
			die "esplora backend on mainnet is blocked; set esplora_url for signet."
		fi
	fi
	CHAIN_TOML=$(printf '[esplora]\nserver_url = "%s"\n' "$ESPLORA_URL")
fi

# ============================================================================
# Phase 5 — Render config, EnvironmentFile, systemd, logrotate
# ============================================================================
log "Writing /etc/ldk-server/config.toml"
# Secrets (bitcoind RPC password, metrics password) are injected via the 0600
# EnvironmentFile below, NOT written here. require_token has no env var, so it
# must live in this 0640 file (group-readable by ldk-server). (OQ2)
METRICS_TOML=""
if [ -n "$METRICS_PASSWORD" ]; then
	METRICS_TOML=$(printf '\n[metrics]\nenabled = true\npoll_metrics_interval = 60\nusername = "metrics"\n# password supplied via EnvironmentFile (LDK_SERVER_METRICS_PASSWORD)\n')
fi
REQUIRE_TOKEN_TOML=""
[ -n "$LSPS2_REQUIRE_TOKEN" ] && REQUIRE_TOKEN_TOML=$(printf 'require_token = "%s"\n' "$LSPS2_REQUIRE_TOKEN")

cat > /etc/ldk-server/config.toml <<EOF
# Generated by linode-stackscript.sh. Built commit: $(cat /etc/ldk-server-build-commit 2>/dev/null || echo unknown)
[node]
network = "${LDK_NETWORK}"
listening_addresses = ["0.0.0.0:9735"]
announcement_addresses = ["${ANNOUNCE_IP}:9735"]
grpc_service_address = "127.0.0.1:3536"   # local-only; reach via SSH tunnel
alias = "${LSP_ALIAS}"

[storage.disk]
dir_path = "/var/lib/ldk-server/"

[log]
level = "Info"                            # NOT Debug/Trace on a funds node
file = "/var/lib/ldk-server/${NETDIR}/ldk-server.log"

${CHAIN_TOML}
[liquidity.lsps2_service]
advertise_service = true
channel_opening_fee_ppm = ${LSPS2_FEE_PPM}
channel_over_provisioning_ppm = 500000
min_channel_opening_fee_msat = ${LSPS2_MIN_FEE_MSAT}
min_channel_lifetime = 4320
max_client_to_self_delay = 1440
min_payment_size_msat = 10000000
max_payment_size_msat = ${LSPS2_MAX_PAYMENT_MSAT}
client_trusts_lsp = false
disable_client_reserve = false
${REQUIRE_TOKEN_TOML}${METRICS_TOML}
EOF
chown root:ldk-server /etc/ldk-server/config.toml; chmod 640 /etc/ldk-server/config.toml

log "Writing 0600 EnvironmentFile with secrets"
{
	echo "# Secrets for ldk-server (root-only; injected by systemd)."
	[ "$CHAIN_BACKEND" = "bitcoind" ] && echo "LDK_SERVER_BITCOIND_RPC_PASSWORD=${BITCOIND_RPC_PASSWORD}"
	[ -n "$METRICS_PASSWORD" ] && echo "LDK_SERVER_METRICS_PASSWORD=${METRICS_PASSWORD}"
} > /etc/ldk-server/ldk-server.env
chown root:root /etc/ldk-server/ldk-server.env; chmod 600 /etc/ldk-server/ldk-server.env

log "Installing systemd unit + drop-ins"
# Base unit is committed in the repo; copy it from the clone.
install -m 0644 "$SRC/contrib/ldk-server.service" /etc/systemd/system/ldk-server.service
install -d -m 0755 /etc/systemd/system/ldk-server.service.d

cat > /etc/systemd/system/ldk-server.service.d/10-environment.conf <<'EOF'
[Service]
EnvironmentFile=/etc/ldk-server/ldk-server.env
EOF

# Hardening drop-in (embedded; deploy/ is not part of the committed repo).
cat > /etc/systemd/system/ldk-server.service.d/20-hardening.conf <<'EOF'
[Service]
UMask=0077
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectKernelLogs=true
ProtectControlGroups=true
ProtectClock=true
ProtectHostname=true
RestrictNamespaces=true
RestrictRealtime=true
RestrictSUIDSGID=true
LockPersonality=true
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
SystemCallFilter=@system-service
SystemCallErrorNumber=EPERM
EOF

log "Installing logrotate (network dir: ${NETDIR})"
cat > /etc/logrotate.d/ldk-server <<EOF
/var/lib/ldk-server/${NETDIR}/ldk-server.log {
    daily
    rotate 14
    compress
    delaycompress
    missingok
    notifempty
    su ldk-server ldk-server
    create 0640 ldk-server ldk-server
    postrotate
        systemctl kill --signal=HUP ldk-server.service
    endscript
}
EOF

systemctl daemon-reload

# ============================================================================
# Phase 6 — Install (but do NOT arm) backup + health-check; first start; handoff
# ============================================================================
log "Installing backup + health-check helpers (not armed)"
install -d -m 0755 /opt/ldk-server-ops
cat > /opt/ldk-server-ops/backup-ldk-server.sh <<EOF
#!/usr/bin/env bash
# Consistent, encrypted, offsite backup of channel state. Configure AGE_RECIPIENT
# and RCLONE_REMOTE, then run via cron/timer as the ldk-server user.
# Does NOT back up keys_mnemonic — copy that offline by hand, once, before funding.
set -euo pipefail
NETWORK_DIR="\${NETWORK_DIR:-/var/lib/ldk-server/${NETDIR}}"
AGE_RECIPIENT="\${AGE_RECIPIENT:-<AGE_PUBLIC_KEY>}"
RCLONE_REMOTE="\${RCLONE_REMOTE:-<rclone-remote>:ldk-server-backups}"
STAMP="\$(date -u +%Y%m%dT%H%M%SZ)"; tmp="\$(mktemp -d)"; trap 'rm -rf "\$tmp"' EXIT
sqlite3 "\$NETWORK_DIR/ldk_node_data.sqlite" ".backup '\$tmp/ldk_node_data.sqlite'"
[ -f "\$NETWORK_DIR/ldk_server_data.sqlite" ] && sqlite3 "\$NETWORK_DIR/ldk_server_data.sqlite" ".backup '\$tmp/ldk_server_data.sqlite'" || true
sqlite3 "\$tmp/ldk_node_data.sqlite" "PRAGMA integrity_check;" | grep -qx ok || { echo "integrity_check failed"; exit 1; }
tar -C "\$tmp" -cf "\$tmp/b.tar" . && age -r "\$AGE_RECIPIENT" -o "\$tmp/b.tar.age" "\$tmp/b.tar"
rclone copy "\$tmp/b.tar.age" "\$RCLONE_REMOTE/ldk-server-\$STAMP.tar.age" --immutable
EOF
chmod 0755 /opt/ldk-server-ops/backup-ldk-server.sh

log "First start (backend=${CHAIN_BACKEND})"
systemctl enable ldk-server
if [ "$CHAIN_BACKEND" = "esplora" ]; then
	# Backend reachable immediately → start now (generates keys_mnemonic, NODE_URI).
	systemctl start ldk-server || log "ldk-server start returned non-zero (check: journalctl -u ldk-server)."
else
	log "bitcoind path: leaving ldk-server enabled-but-stopped until IBD completes (see NEXT_STEPS)."
fi

# --- Operator handoff (seed-free) -------------------------------------------
START_HINT="The node was started; the seed file now exists."
if [ "$CHAIN_BACKEND" = "bitcoind" ]; then
	START_HINT="Wait for bitcoind sync (bitcoin-cli -datadir=/var/lib/bitcoind getblockchaininfo => initialblockdownload=false),
then: sudo systemctl start ldk-server   # this generates the seed file"
fi

cat > /root/NEXT_STEPS.txt <<EOF
================================================================================
 ldk-server LSPS2 LSP — provisioning complete. THE NODE IS UNFUNDED.
================================================================================
Network: ${NETWORK_UDF} (${LDK_NETWORK})   Backend: ${CHAIN_BACKEND}
Built commit: $(cat /etc/ldk-server-build-commit 2>/dev/null || echo unknown)

HARD RULES (funds safety):
 * NO sats touch this node until the mnemonic is backed up offline AND a
   restore has been proven on a separate clean host.
 * gRPC (127.0.0.1:3536) is local-only. Reach it via SSH tunnel:
       ssh -L 3536:localhost:3536 ${SSH_USER}@${ANNOUNCE_IP}
   Never bind it to 0.0.0.0 (that exposes /metrics + full node control).
 * Never run two instances on the same identity (= fund loss).

NEXT STEPS:
 1. ${START_HINT}
 2. Back up the seed OFFLINE, by hand:  /var/lib/ldk-server/keys_mnemonic
    (24 words; 0600. Do NOT copy it into the recurring backup job.)
 3. Configure + prove a restore: edit /opt/ldk-server-ops/backup-ldk-server.sh
    (set AGE_RECIPIENT, RCLONE_REMOTE), run it, and restore on a CLEAN host.
 4. Only then fund:  ldk-server-cli onchain-receive   (mainnet: <=0.05 BTC pilot)
 5. Distribute the require_token to known clients out-of-band (if set).

Check status:  systemctl status ldk-server ; journalctl -u ldk-server -f
StackScript log: /var/log/stackscript.log
Upgrade later (as root): cd ${SRC} && git pull && git checkout <commit> \\
    && CARGO_HOME=/opt/cargo RUSTUP_HOME=/opt/rust /opt/cargo/bin/cargo build \\
    --release --features experimental-lsps2-support \\
    && install -m0755 target/release/ldk-server* /usr/local/bin/ \\
    && systemctl restart ldk-server   # back up FIRST
================================================================================
EOF
chmod 600 /root/NEXT_STEPS.txt
printf '\n*** ldk-server LSP deployed and UNFUNDED. Read /root/NEXT_STEPS.txt before funding. ***\n\n' > /etc/motd

log "Done. Node is UNFUNDED. See /root/NEXT_STEPS.txt"
