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
# ============================================================================

# --- UDF (deploy-time form fields; required unless a default is given) ------
# <UDF name="ssh_user" label="Admin username (non-root sudo)" default="lsp" />
# <UDF name="ssh_pubkey" label="Admin SSH public key (Ed25519 recommended)" />
# <UDF name="ssh_allowed_ips" label="Restrict SSH to these IPs (CIDR, comma-separated; blank = any)" default="" />
# <UDF name="network" label="Bitcoin network (mainnet = self-hosted bitcoind; Mutinynet = esplora)" oneOf="mainnet,mutinynet" default="mutinynet" />
# <UDF name="esplora_url" label="Esplora URL (Mutinynet only; blank = https://mutinynet.com/api)" default="" />
# <UDF name="lsp_alias" label="LSP node alias (<=32 chars)" default="ldk-lsp" />
# <UDF name="announce_ip" label="Announcement IPv4 (blank = auto-detect this Linode's public IP)" default="" />
# <UDF name="lsps2_require_token" label="LSPS2 require_token (gate to known clients; blank = open)" default="" />
# <UDF name="lsps2_channel_opening_fee_ppm" label="LSPS2 opening fee (ppm)" default="1000" />
# <UDF name="lsps2_min_channel_opening_fee_msat" label="LSPS2 min opening fee (msat)" default="10000000" />
# <UDF name="lsps2_max_payment_size_msat" label="LSPS2 max payment size (msat)" default="330000000" />
# <UDF name="metrics_password" label="Prometheus /metrics Basic-Auth password (masked; blank = metrics off)" default="" />

# --- Strict mode + logging --------------------------------------------------
set -euo pipefail
# StackScript stdout is NOT streamed; persist it. UDF values arrive as env vars
# and this log would otherwise be world-readable, so lock it down immediately.
LOGFILE=/var/log/stackscript.log
umask 077
touch "$LOGFILE"   # created 0600 under the umask
exec > >(tee -ai "$LOGFILE") 2>&1

# Pinned ldk-server commit (no release tags exist; version is 0.1.0).
# MUST be a full 40-char SHA: abbreviations can be shadowed by a hostile
# refname of the same spelling, and the post-checkout verification below
# compares rev-parse output against this exact value.
LDK_SERVER_REPO="https://github.com/lightningdevkit/ldk-server.git"
LDK_SERVER_COMMIT="c8424dbdd739a99f0bb8b2dd525674dd20a48ef2"
# Pinned Rust toolchain + rustup-init release (same input → same toolchain
# months later). SHA-256 sums are the published rustup-init.sha256 values for
# RUSTUP_VERSION; bump all three together.
RUST_TOOLCHAIN="1.97.1"
RUSTUP_VERSION="1.29.0"
RUSTUP_INIT_SHA256_X86_64="4acc9acc76d5079515b46346a485974457b5a79893cfb01112423c89aeb5aa10"
RUSTUP_INIT_SHA256_AARCH64="9732d6c5e2a098d3521fca8145d826ae0aaa067ef2385ead08e6feac88fa5792"
# bitcoind release used for the self-hosted backend (operator may bump).
BITCOIND_VERSION="29.0"
# Bitcoin Core guix builder keys, pinned by primary PGP fingerprint (from the
# bitcoin-core/guix.sigs repo). SHA256SUMS must carry at least
# BITCOIND_GPG_MIN_SIGS valid signatures from these keys — a hard gate; see
# verify_bitcoind_sigs(). The bitcoind backend is mainnet-only (real funds).
BITCOIND_GPG_MIN_SIGS=2
BITCOIND_BUILDER_KEYS="152812300785C96444D3334D17565732E08E5E41 achow101
E777299FC265DD04793070EB944D35F9AC3DB76A fanquake
6B002C6EA3F91B1B0DF0C9BC8F617F1200A6D25C glozow
D1DBF2C4B96F2DEBF4C16654410108112E7EA81F hebasto
67AA5B46E7AF78053167FE343B8F814A784218F8 willcl-ark
A8FC55F3B04BA3146F3492E79303B33A305224CB TheCharlatan"
# Mutinynet (custom signet) parameters — see https://blog.mutinywallet.com/mutinynet/
# Mutinynet is reachable only via esplora here: its 30 s blocks require a custom
# bitcoind build, so the chain backend is derived from the network below.
MUTINYNET_ESPLORA="https://mutinynet.com/api"

MIN_RAM_MB=7800   # require ~8GB; fat-LTO link must fit in RAM (OQ6)

# --- Helpers ----------------------------------------------------------------
log()  { echo "[stackscript $(date -u +%H:%M:%S)] $*"; }
die()  { trap - ERR; echo "[stackscript FATAL] $*" >&2; echo "FAILED: $*" > /root/STACKSCRIPT_FAILED.txt; rm -f /root/STACKSCRIPT_OK; exit 1; }

# Any UNGUARDED failure (apt/rustup/clone/cargo/systemctl with no explicit `|| die`)
# must leave a clear failure marker — never a silently half-provisioned box that
# looks done. The trap disables itself inside die() to avoid recursion.
trap 'die "unexpected failure (rc=$?) near line $LINENO"' ERR

# Read a UDF value (Linode injects each UDF as an env var of the exact same
# name), falling back to a default. Usage: v=$(udf network mainnet)
udf() { printf '%s' "${!1:-${2-}}"; }

# --- Read UDF values --------------------------------------------------------
SSH_USER=$(udf ssh_user lsp)
SSH_PUBKEY=$(udf ssh_pubkey "")
SSH_ALLOWED_IPS=$(udf ssh_allowed_ips "")
NETWORK_UDF=$(udf network mutinynet)
ESPLORA_URL=$(udf esplora_url "")
LSP_ALIAS=$(udf lsp_alias ldk-lsp)
ANNOUNCE_IP=$(udf announce_ip "")
LSPS2_REQUIRE_TOKEN=$(udf lsps2_require_token "")
LSPS2_FEE_PPM=$(udf lsps2_channel_opening_fee_ppm 1000)
LSPS2_MIN_FEE_MSAT=$(udf lsps2_min_channel_opening_fee_msat 10000000)
LSPS2_MAX_PAYMENT_MSAT=$(udf lsps2_max_payment_size_msat 330000000)
METRICS_PASSWORD=$(udf metrics_password "")

# Map the friendly network name to ldk-server's Network enum value, and DERIVE
# the chain backend — it is a function of the network, not a choice:
#  * mainnet must self-host bitcoind: gossip UTXO verification is disabled for
#    esplora/electrum (as_utxo_source() -> None), so a mainnet LSP needs its
#    own non-pruned bitcoind. (OQ1)
#  * Mutinynet must use esplora: its 30 s block cadence needs a custom bitcoind
#    build (signetblocktime is not a stock Core option), and this script
#    installs the official bitcoincore.org binary. (OQ4)
case "$NETWORK_UDF" in
	mainnet)   LDK_NETWORK="bitcoin"; NETDIR="bitcoin"; CHAIN_BACKEND="bitcoind" ;;
	mutinynet) LDK_NETWORK="signet";  NETDIR="signet";  CHAIN_BACKEND="esplora" ;;
	*) die "Unknown network '$NETWORK_UDF' (expected mainnet or mutinynet)." ;;
esac

# ============================================================================
# Phase 1 — Validate inputs, plan, and host
# ============================================================================
log "Validating inputs"

[ "$(id -u)" -eq 0 ] || die "StackScript must run as root."
command -v apt-get >/dev/null 2>&1 || die "Unsupported image: apt-get not found (use Debian/Ubuntu)."

# SSH key sanity — a bad/empty key locks the operator out of the new user, and
# an embedded newline would smuggle extra authorized_keys entries (or options
# like command="...") past a visual "my key is there" check. Enforce a strict
# single-line "<type> <base64> [comment]" shape with no control characters.
case "$SSH_PUBKEY" in
	*$'\n'*|*$'\r'*) die "ssh_pubkey must be a single line (embedded newlines are not allowed)." ;;
esac
_pubkey_re='^(ssh-ed25519|ssh-rsa|ecdsa-sha2-[a-z0-9-]+|sk-ssh-ed25519@openssh\.com) [A-Za-z0-9+/=]+( [^[:cntrl:]]*)?$'
[[ "$SSH_PUBKEY" =~ $_pubkey_re ]] || die "ssh_pubkey is empty or not a recognized single-line OpenSSH public key."

# Numeric LSPS2 fields.
for v in "$LSPS2_FEE_PPM" "$LSPS2_MIN_FEE_MSAT" "$LSPS2_MAX_PAYMENT_MSAT"; do
	[[ "$v" =~ ^[0-9]+$ ]] || die "LSPS2 numeric field has a non-numeric value: '$v'."
done
# min_payment_size_msat is hard-coded to 10000000 in the config; reject a smaller max.
[ "$LSPS2_MAX_PAYMENT_MSAT" -ge 10000000 ] || die "lsps2_max_payment_size_msat must be >= 10000000 (the min payment size)."

# Free-text UDFs flow into config.toml / bitcoin.conf / sshd / ufw. Reject quotes,
# backslashes, and newlines (TOML/config injection) and enforce per-field shapes.
# Without this, e.g. a require_token containing a quote+newline could inject a
# funds-relevant LSPS2 setting into the rendered TOML.
# reject_special is load-bearing for metrics_password (which has no whitelist
# regex) and a deliberate backstop for the whitelisted fields below, should a
# future change relax one of their regexes.
reject_special() { case "$2" in *['"'\\]*|*$'\n'*|*$'\r'*) die "$1 must not contain quotes, backslashes, or newlines." ;; esac; }
reject_special "ssh_user" "$SSH_USER"
reject_special "lsp_alias" "$LSP_ALIAS"
reject_special "esplora_url" "$ESPLORA_URL"
reject_special "lsps2_require_token" "$LSPS2_REQUIRE_TOKEN"
# metrics_password lands in the systemd EnvironmentFile, where whitespace and
# control characters parse differently than the raw value (or inject extra
# KEY=value lines). Reject them outright so the value provably round-trips.
reject_special "metrics_password" "$METRICS_PASSWORD"
case "$METRICS_PASSWORD" in
	*[[:space:]]*|*[[:cntrl:]]*) die "metrics_password must not contain whitespace or control characters." ;;
esac

[[ "$SSH_USER" =~ ^[a-z_][a-z0-9_-]*$ ]] || die "ssh_user must match ^[a-z_][a-z0-9_-]*$ (got '$SSH_USER')."
[ "${#LSP_ALIAS}" -le 32 ] || die "lsp_alias must be at most 32 bytes."
_alias_re='^[A-Za-z0-9 ._-]+$'
[[ "$LSP_ALIAS" =~ $_alias_re ]] || die "lsp_alias may only contain letters, digits, space, '.', '_', '-'."
if [ -n "$LSPS2_REQUIRE_TOKEN" ]; then
	[[ "$LSPS2_REQUIRE_TOKEN" =~ ^[A-Za-z0-9._-]+$ ]] || die "lsps2_require_token may only contain [A-Za-z0-9._-]."
fi
if [ -n "$ESPLORA_URL" ]; then
	[[ "$ESPLORA_URL" =~ ^https?://[A-Za-z0-9./:_-]+$ ]] || die "esplora_url must be an http(s) URL."
fi

# Mainnet LSP must gate the pilot — refuse an open service on real funds.
if [ "$LDK_NETWORK" = "bitcoin" ] && [ -z "$LSPS2_REQUIRE_TOKEN" ]; then
	die "mainnet deployment requires a non-empty lsps2_require_token (gate the pilot to known clients)."
fi

# RAM floor for the fat-LTO build (OQ6).
RAM_MB=$(awk '/MemTotal/ {printf "%d", $2/1024}' /proc/meminfo)
log "Detected ${RAM_MB} MB RAM"
[ "${RAM_MB:-0}" -ge "$MIN_RAM_MB" ] || die "Need >= 8 GB RAM to build (have ${RAM_MB} MB). Pick a larger Linode plan."

# Resolve the announcement IP if blank. Linodes carry their public IPv4 on the
# interface (no NAT), so reading it locally suffices — no external lookup.
if [ -z "$ANNOUNCE_IP" ]; then
	ANNOUNCE_IP=$(ip -4 -o addr show scope global 2>/dev/null | awk '{print $4}' | cut -d/ -f1 | head -1)
fi
[ -n "$ANNOUNCE_IP" ] || die "Could not determine a public IPv4 for announcement_addresses; set the announce_ip UDF."
[[ "$ANNOUNCE_IP" =~ ^([0-9]{1,3}\.){3}[0-9]{1,3}$ ]] || die "announce_ip is not a valid IPv4 address: '$ANNOUNCE_IP'."
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
fi
# Asserted OUTSIDE the creation guard (each line is idempotent): a re-run — or
# a first run that died between swapon and here — must still persist swap in
# fstab and (re-)apply the swappiness tuning.
grep -q '^/swapfile ' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
printf 'vm.swappiness=10\n' > /etc/sysctl.d/99-ldk-swap.conf
sysctl --system >/dev/null 2>&1 || true

log "Installing packages"
apt-get update -y
apt-get install -y --no-install-recommends \
	ca-certificates curl git build-essential pkg-config sudo openssl gpg \
	ufw fail2ban unattended-upgrades \
	sqlite3 age rclone xxd

log "Creating admin user '${SSH_USER}'"
if ! id -u "$SSH_USER" >/dev/null 2>&1; then
	adduser --disabled-password --gecos "" "$SSH_USER"
fi
usermod -aG sudo "$SSH_USER"

# The account has a disabled password, but Debian's %sudo rule still demands the
# invoking user's password — so sudo would be unusable and every documented
# post-provision step (starting the node, seed backup, upgrades) would be blocked
# over SSH. Grant passwordless sudo instead: key-only SSH is the auth boundary,
# the standard posture for key-only cloud images. Validate with visudo before
# install — a malformed drop-in would break sudo for the whole host.
log "Granting passwordless sudo to '${SSH_USER}' (key-only host; no password exists)"
SUDOERS_TMP=$(mktemp)
printf '%s ALL=(ALL) NOPASSWD:ALL\n' "$SSH_USER" > "$SUDOERS_TMP"
visudo -cf "$SUDOERS_TMP" >/dev/null || { rm -f "$SUDOERS_TMP"; die "Generated sudoers drop-in failed visudo validation; not installing it."; }
install -m 440 -o root -g root "$SUDOERS_TMP" "/etc/sudoers.d/90-${SSH_USER}"
rm -f "$SUDOERS_TMP"

install -d -m 700 -o "$SSH_USER" -g "$SSH_USER" "/home/$SSH_USER/.ssh"
printf '%s\n' "$SSH_PUBKEY" > "/home/$SSH_USER/.ssh/authorized_keys"
chmod 600 "/home/$SSH_USER/.ssh/authorized_keys"
chown "$SSH_USER:$SSH_USER" "/home/$SSH_USER/.ssh/authorized_keys"
# Cross-check with the real parser: exactly one valid key must have landed.
_keycount=$(ssh-keygen -lf "/home/$SSH_USER/.ssh/authorized_keys" 2>/dev/null | wc -l) || true
[ "${_keycount:-0}" -eq 1 ] || die "authorized_keys must contain exactly one valid SSH key (ssh-keygen found ${_keycount:-0})."

log "Hardening sshd (key-only, no root)"
# sshd uses first-obtained-value-wins and includes sshd_config.d/*.conf in lexical
# order, so this drop-in must sort before anything the image ships (e.g. cloud-init's
# 50-cloud-init.conf setting "PasswordAuthentication yes") or it is silently ignored.
SSH_DROPIN=/etc/ssh/sshd_config.d/00-ldk-hardening.conf
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
# Assert the *effective* merged config: `sshd -t` only checks syntax, so another
# drop-in could still override the hardening without any error.
sshd -T | grep -qi '^passwordauthentication no' \
	|| die "sshd hardening not effective: PasswordAuthentication is still enabled by another config file."
sshd -T | grep -qi '^permitrootlogin no' \
	|| die "sshd hardening not effective: PermitRootLogin is still enabled by another config file."
# Don't mask a reload failure entirely (the config is sshd -t-validated, so a
# failure here is informative) — but don't die either: on socket-activated
# images the unit may be inactive, and the validated config still applies to
# the next sshd (re)start/connection.
systemctl reload ssh 2>/dev/null || systemctl reload sshd 2>/dev/null \
	|| log "WARNING: could not reload ssh/sshd (unit inactive or named differently); the validated config applies on the next sshd start."

log "Configuring UFW (SSH + 9735 inbound only)"
ufw --force reset >/dev/null
ufw default deny incoming >/dev/null
ufw default allow outgoing >/dev/null
if [ -n "$SSH_ALLOWED_IPS" ]; then
	_valid=0
	IFS=',' read -ra _ips <<< "$SSH_ALLOWED_IPS"
	for ip in "${_ips[@]}"; do
		ip=$(echo "$ip" | xargs)
		[ -n "$ip" ] || continue
		if [[ "$ip" =~ ^([0-9]{1,3}\.){3}[0-9]{1,3}(/[0-9]{1,2})?$ ]] || [[ "$ip" =~ ^[0-9A-Fa-f:]+(/[0-9]{1,3})?$ ]]; then
			ufw allow from "$ip" to any port 22 proto tcp >/dev/null
			_valid=$((_valid + 1))
		else
			log "WARNING: ignoring invalid ssh_allowed_ips entry '$ip'."
		fi
	done
	# Never lock the operator out: if nothing valid parsed, fall back to open SSH.
	[ "$_valid" -gt 0 ] || { log "WARNING: no valid ssh_allowed_ips entries; allowing SSH from any IP."; ufw allow 22/tcp >/dev/null; }
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
# Force the systemd journal backend: minimal images ship no rsyslog/auth.log,
# where the default sshd jail fails to start — and a masked failure would leave
# a documented security control silently absent. No mask: fail loudly instead.
install -d -m 755 /etc/fail2ban/jail.d
printf '[DEFAULT]\nbackend = systemd\n' > /etc/fail2ban/jail.d/90-ldk-systemd-backend.conf
systemctl enable --now fail2ban || die "fail2ban failed to start (see 'journalctl -u fail2ban'); refusing to continue without it."
systemctl is-active --quiet fail2ban || die "fail2ban is not active after start; refusing to continue without it."

# ============================================================================
# Phase 3 — Build ldk-server from source (pinned commit)
# ============================================================================
# The toolchain install and cargo build run as a throwaway non-root 'builder'
# user so no transitive build.rs/proc-macro executes with root privileges on a
# box that will custody funds; root only installs the finished binaries.
# rustup-init is a pinned release verified against its published SHA-256 —
# no unpinned `curl | sh`.
log "Creating throwaway build user 'builder'"
id -u builder >/dev/null 2>&1 || useradd --system --create-home --home-dir /opt/builder --shell /usr/sbin/nologin builder

log "Installing Rust ${RUST_TOOLCHAIN} via rustup-init ${RUSTUP_VERSION} (as builder)"
RUSTUP_HOME=/opt/rust
CARGO_HOME=/opt/cargo
install -d -m 755 -o builder -g builder "$RUSTUP_HOME" "$CARGO_HOME"
arch=$(uname -m); case "$arch" in
	x86_64)  rust_triple=x86_64-unknown-linux-gnu;  rustup_sha="$RUSTUP_INIT_SHA256_X86_64" ;;
	aarch64) rust_triple=aarch64-unknown-linux-gnu; rustup_sha="$RUSTUP_INIT_SHA256_AARCH64" ;;
	*) die "Unsupported arch $arch for the Rust toolchain." ;;
esac
tmp=$(mktemp -d); chmod 755 "$tmp"   # builder must be able to read rustup-init
curl --proto '=https' --tlsv1.2 -fsS -o "$tmp/rustup-init" \
	"https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/${rust_triple}/rustup-init"
echo "${rustup_sha}  ${tmp}/rustup-init" | sha256sum -c - >/dev/null || die "rustup-init SHA-256 mismatch."
chmod 755 "$tmp/rustup-init"
runuser -u builder -- env HOME=/opt/builder RUSTUP_HOME="$RUSTUP_HOME" CARGO_HOME="$CARGO_HOME" \
	"$tmp/rustup-init" -y --no-modify-path --default-toolchain "$RUST_TOOLCHAIN"
rm -rf "$tmp"

log "Cloning ldk-server @ ${LDK_SERVER_COMMIT} and building (this takes a while)"
# Build in a throwaway temp clone: only the binaries, the base systemd unit,
# and /etc/ldk-server-build-commit persist from it. /opt/ldk-server-src is the
# OPERATOR's upgrade clone (see NEXT_STEPS) — a manual re-run of this script
# must never rm -rf an in-place upgrade the operator has staged there.
SRC=/opt/ldk-server-src
BUILD_SRC=$(mktemp -d); chmod 755 "$BUILD_SRC"   # builder must be able to read it
git clone "$LDK_SERVER_REPO" "$BUILD_SRC"
# Detached checkout of the pin, then verify HEAD is EXACTLY the pinned commit —
# a branch/tag named like the pin (refname shadowing) must not change the build.
git -C "$BUILD_SRC" checkout --detach "$LDK_SERVER_COMMIT" --
[ "$(git -C "$BUILD_SRC" rev-parse "HEAD^{commit}")" = "$LDK_SERVER_COMMIT" ] \
	|| die "Checkout mismatch: HEAD is not the pinned commit ${LDK_SERVER_COMMIT}. Refusing to build."
# Record the commit while root still owns the clone (root git refuses
# builder-owned repos: safe.directory). Kept in a var for later reuse and
# persisted to /etc for post-provision inspection.
BUILD_COMMIT=$(git -C "$BUILD_SRC" rev-parse HEAD)
printf '%s\n' "$BUILD_COMMIT" > /etc/ldk-server-build-commit
chown -R builder:builder "$BUILD_SRC"
# --locked: enforce the committed Cargo.lock (fail loudly if stale) on a funds node.
( cd "$BUILD_SRC" && runuser -u builder -- env HOME=/opt/builder RUSTUP_HOME="$RUSTUP_HOME" CARGO_HOME="$CARGO_HOME" \
	PATH="$CARGO_HOME/bin:$PATH" cargo build --release --locked --features experimental-lsps2-support )

install -m 0755 "$BUILD_SRC/target/release/ldk-server" /usr/local/bin/ldk-server
install -m 0755 "$BUILD_SRC/target/release/ldk-server-cli" /usr/local/bin/ldk-server-cli
# Base systemd unit is committed in the repo; grab it before the build tree goes.
install -m 0644 "$BUILD_SRC/contrib/ldk-server.service" /etc/systemd/system/ldk-server.service
rm -rf "$BUILD_SRC"
log "Built commit: ${BUILD_COMMIT}"

# Seed the operator's upgrade clone only if absent — never touch an existing
# tree (it may hold the operator's in-place upgrade state).
if [ ! -e "$SRC" ]; then
	git clone "$LDK_SERVER_REPO" "$SRC"
	git -C "$SRC" checkout --detach "$LDK_SERVER_COMMIT" --
	chown -R builder:builder "$SRC"
else
	log "Existing ${SRC} found; leaving it untouched (operator upgrade clone)."
fi

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

	# Mainnet hard gate: SHA-256 alone only proves the tarball matches a sums
	# file served by the SAME origin — a compromised/MITM'd mirror can forge
	# both. The guix builder signatures over SHA256SUMS cannot, and this
	# bitcoind is the node's own chain source for real funds. Runs inside the
	# download subshell ($PWD holds SHA256SUMS; GNUPGHOME dies with it).
	verify_bitcoind_sigs() {
		local base="$1" good=0 fpr name
		curl -fsSLO "${base}/SHA256SUMS.asc" || { log "ERROR: could not fetch SHA256SUMS.asc."; return 1; }
		export GNUPGHOME="$PWD/gnupg"
		install -d -m 700 "$GNUPGHOME"
		while read -r fpr name; do
			# Key bodies come from the guix.sigs repo, but only signatures whose
			# PRIMARY fingerprint matches a pin below count toward the threshold,
			# so a tampered key file cannot satisfy the gate.
			if curl -fsSL "https://raw.githubusercontent.com/bitcoin-core/guix.sigs/main/builder-keys/${name}.gpg" -o "${name}.gpg"; then
				gpg --batch --import "${name}.gpg" >/dev/null 2>&1 || log "WARNING: could not import builder key ${name}."
			else
				log "WARNING: could not download builder key ${name}."
			fi
		done <<< "$BITCOIND_BUILDER_KEYS"
		# No --batch: gpg >= 2.5 aborts a multi-signature verify at the first
		# signature from a key it does not have when run with --batch.
		gpg --status-file gpg-status.txt --verify SHA256SUMS.asc SHA256SUMS >/dev/null 2>&1 || true
		while read -r fpr name; do
			if grep -q "VALIDSIG .* ${fpr}\$" gpg-status.txt 2>/dev/null; then
				log "Good SHA256SUMS signature from builder ${name} (${fpr})"
				good=$((good + 1))
			fi
		done <<< "$BITCOIND_BUILDER_KEYS"
		[ "$good" -ge "$BITCOIND_GPG_MIN_SIGS" ] || { log "ERROR: only ${good} valid pinned builder signature(s) on SHA256SUMS (need >= ${BITCOIND_GPG_MIN_SIGS}). Refusing bitcoind on mainnet."; return 1; }
	}

	# Download Bitcoin Core; SHA256-verify AND require pinned builder signatures
	# over SHA256SUMS (hard gate, see above — this path is mainnet-only).
	arch=$(uname -m); case "$arch" in x86_64) btarch=x86_64-linux-gnu ;; aarch64) btarch=aarch64-linux-gnu ;; *) die "Unsupported arch $arch for bitcoind." ;; esac
	tmp=$(mktemp -d); ( cd "$tmp"
		base="https://bitcoincore.org/bin/bitcoin-core-${BITCOIND_VERSION}"
		curl -fsSLO "${base}/bitcoin-${BITCOIND_VERSION}-${btarch}.tar.gz"
		curl -fsSLO "${base}/SHA256SUMS"
		verify_bitcoind_sigs "$base" || exit 1
		grep " bitcoin-${BITCOIND_VERSION}-${btarch}.tar.gz\$" SHA256SUMS | sha256sum -c - || exit 1
		tar -xzf "bitcoin-${BITCOIND_VERSION}-${btarch}.tar.gz"
		install -m 0755 "bitcoin-${BITCOIND_VERSION}/bin/bitcoind" "bitcoin-${BITCOIND_VERSION}/bin/bitcoin-cli" /usr/local/bin/
	) || die "bitcoind download/verify/install failed."
	rm -rf "$tmp"

	# rpcauth (salted) — ldk-server has no cookie-auth path, only user/password.
	# The RPC password is generated on-box, never taken as a UDF: it is used
	# exactly once, machine-to-machine on loopback (the operator's bitcoin-cli
	# uses cookie auth), and a UDF value could carry newlines that inject env
	# vars into ldk-server.env or break the rpcauth HMAC. Hex only => safe in
	# both bitcoin.conf and the EnvironmentFile.
	RPC_USER="ldkserver"
	if [ -f /etc/bitcoin/bitcoin.conf ]; then
		# Re-run safety: operators tune bitcoin.conf (prune, dbcache, custom
		# datadir/Block Storage); regenerating it would silently destroy those
		# edits and rotate rpcauth out from under a working node. Keep the file
		# and reuse the existing RPC password so the preserved rpcauth line
		# still matches what ldk-server sends.
		log "Existing /etc/bitcoin/bitcoin.conf found; preserving it (re-run)."
		BITCOIND_RPC_PASSWORD=$(sed -n 's/^LDK_SERVER_BITCOIND_RPC_PASSWORD=//p' /etc/ldk-server/ldk-server.env 2>/dev/null | head -n1)
		[ -n "$BITCOIND_RPC_PASSWORD" ] || die "/etc/bitcoin/bitcoin.conf exists but LDK_SERVER_BITCOIND_RPC_PASSWORD is missing from /etc/ldk-server/ldk-server.env; restore that file, or remove bitcoin.conf to regenerate both."
	else
		BITCOIND_RPC_PASSWORD=$(head -c16 /dev/urandom | xxd -p)
		rpcsalt=$(head -c16 /dev/urandom | xxd -p)
		rpchmac=$(printf '%s' "$BITCOIND_RPC_PASSWORD" | openssl dgst -sha256 -hmac "$rpcsalt" | awk '{print $NF}')

		{
			echo "# Managed by ldk-server linode-stackscript. Loopback RPC only."
			# bitcoind backend is mainnet-only: the backend is derived from the
			# network (stock Core can't follow Mutinynet's custom signet).
			echo "chain=main"
			echo "server=1"
			echo "daemon=0"
			echo "txindex=0"
			echo "dbcache=2048"
			echo "rpcbind=127.0.0.1"
			echo "rpcallowip=127.0.0.1"
			echo "rpcauth=${RPC_USER}:${rpcsalt}\$${rpchmac}"
		} > /etc/bitcoin/bitcoin.conf
		chown root:bitcoin /etc/bitcoin/bitcoin.conf; chmod 640 /etc/bitcoin/bitcoin.conf
	fi

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
# Shutdown flush of dbcache=2048 can take minutes (worse during/after IBD);
# systemd's default 90 s stop timeout would SIGKILL bitcoind mid-flush and
# corrupt the chainstate (=> days-long reindex). Give it time to stop cleanly.
TimeoutStopSec=600
KillMode=mixed
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=full
ReadWritePaths=/var/lib/bitcoind
[Install]
WantedBy=multi-user.target
EOF
	# RPC is loopback-bound and the default UFW policy denies all inbound, so the
	# RPC port (8332, mainnet) is never exposed.
	systemctl daemon-reload
	systemctl enable --now bitcoind
	RPC_PORT=8332
	CHAIN_TOML=$(printf '[bitcoind]\nrpc_address = "127.0.0.1:%s"\nrpc_user = "%s"\n# rpc_password supplied via EnvironmentFile (LDK_SERVER_BITCOIND_RPC_PASSWORD)\n' "$RPC_PORT" "$RPC_USER")
else
	# Remote esplora (Mutinynet-only path). Default to the Mutinynet endpoint.
	[ -n "$ESPLORA_URL" ] || ESPLORA_URL="$MUTINYNET_ESPLORA"
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
# Generated by linode-stackscript.sh. Built commit: ${BUILD_COMMIT}
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

log "Installing systemd drop-ins"
# The base unit was installed from the build clone in Phase 3.
install -d -m 0755 /etc/systemd/system/ldk-server.service.d

cat > /etc/systemd/system/ldk-server.service.d/10-environment.conf <<'EOF'
[Service]
EnvironmentFile=/etc/ldk-server/ldk-server.env
EOF

# bitcoind path: order after bitcoind AND gate ldk-server startup on bitcoind's
# RPC actually answering. After a reboot bitcoind is "started" (Type=simple)
# long before RPC is up (block-index load), so without the gate ldk-server
# would crash-loop every 10 s. The '+' prefix runs bitcoin-cli with full
# privileges so it can read the RPC cookie in /var/lib/bitcoind (loopback RPC
# only; nothing is exposed). The base unit orders after network-online alone;
# referencing bitcoind.service there would be dead config on esplora.
if [ "$CHAIN_BACKEND" = "bitcoind" ]; then
	cat > /etc/systemd/system/ldk-server.service.d/15-bitcoind.conf <<'EOF'
[Unit]
After=bitcoind.service
[Service]
ExecStartPre=+/usr/local/bin/bitcoin-cli -conf=/etc/bitcoin/bitcoin.conf -datadir=/var/lib/bitcoind -rpcwait -rpcwaittimeout=600 getblockchaininfo
TimeoutStartSec=660
EOF
fi

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
#
# ⚠️  STALE-RESTORE WARNING: a snapshot goes stale the moment channel state
#     advances. Restoring a stale snapshot on a funded node and letting it run
#     (or force-close) broadcasts since-revoked commitments — the counterparty
#     sweeps the channels via penalty transactions. After any post-funding
#     restore, do NOT let the node act (esp. force-close) before assessing.
#     The snapshot cadence bounds the loss window, so run this often (sqlite
#     .backup is cheap); live replication (e.g. VSS) is the long-term answer.
set -euo pipefail
NETWORK_DIR="\${NETWORK_DIR:-/var/lib/ldk-server/${NETDIR}}"
AGE_RECIPIENT="\${AGE_RECIPIENT:-<AGE_PUBLIC_KEY>}"
RCLONE_REMOTE="\${RCLONE_REMOTE:-<rclone-remote>:ldk-server-backups}"
STAMP="\$(date -u +%Y%m%dT%H%M%SZ)"; tmp="\$(mktemp -d)"; trap 'rm -rf "\$tmp"' EXIT
# Stage DB copies in \$tmp/data so tar never archives its own output tree.
mkdir "\$tmp/data"
sqlite3 "\$NETWORK_DIR/ldk_node_data.sqlite" ".backup '\$tmp/data/ldk_node_data.sqlite'"
[ -f "\$NETWORK_DIR/ldk_server_data.sqlite" ] && sqlite3 "\$NETWORK_DIR/ldk_server_data.sqlite" ".backup '\$tmp/data/ldk_server_data.sqlite'" || true
for db in "\$tmp/data"/*.sqlite; do
	sqlite3 "\$db" "PRAGMA integrity_check;" | grep -qx ok || { echo "integrity_check failed: \$db"; exit 1; }
done
tar -C "\$tmp/data" -cf "\$tmp/b.tar" . && age -r "\$AGE_RECIPIENT" -o "\$tmp/b.tar.age" "\$tmp/b.tar"
# copyto, not copy: the destination is the object name, so the remote holds
# flat ldk-server-<STAMP>.tar.age objects (copy would nest b.tar.age inside).
rclone copyto "\$tmp/b.tar.age" "\$RCLONE_REMOTE/ldk-server-\$STAMP.tar.age"
EOF
chmod 0755 /opt/ldk-server-ops/backup-ldk-server.sh

# Health check: units active + disk headroom. Installed but NOT armed — wire it
# to a cron/systemd timer (and alerting, e.g. a systemd OnFailure= hook) once
# the node is enabled. A mainnet chainstate grows toward ~600 GB; without disk
# alerting the box fills up and the node crash-loops.
cat > /opt/ldk-server-ops/health-check.sh <<'EOF'
#!/usr/bin/env bash
# Host health check for the ldk-server LSP deployment.
# Exit codes: 0 = healthy, 1 = one or more checks failed (details on stdout).
# Arm only after the node is enabled (bitcoind path: post-IBD, NEXT_STEPS step 1);
# before that, an inactive ldk-server is expected and would report as a failure.
set -u
DISK_MAX_PCT="${DISK_MAX_PCT:-90}"
rc=0
for unit in ldk-server bitcoind; do
	# Only check units this deployment installed (esplora boxes have no bitcoind).
	[ -f "/etc/systemd/system/${unit}.service" ] || continue
	if systemctl is-active --quiet "$unit"; then
		echo "OK: $unit is active"
	else
		echo "FAIL: $unit is not active (check: systemctl status $unit)"; rc=1
	fi
done
for dir in /var/lib/ldk-server /var/lib/bitcoind; do
	[ -d "$dir" ] || continue
	pct=$(df -P "$dir" | awk 'NR==2 {sub("%","",$5); print $5}')
	if [ "${pct:-100}" -ge "$DISK_MAX_PCT" ]; then
		echo "FAIL: $dir filesystem is ${pct:-?}% full (threshold ${DISK_MAX_PCT}%)"; rc=1
	else
		echo "OK: $dir filesystem is ${pct}% full"
	fi
done
exit "$rc"
EOF
chmod 0755 /opt/ldk-server-ops/health-check.sh

log "First start (backend=${CHAIN_BACKEND})"
START_FAILED=0
if [ "$CHAIN_BACKEND" = "esplora" ]; then
	# Backend reachable immediately → start now (generates keys_mnemonic, NODE_URI).
	# Do NOT mask a real failure: a broken node must not look like a successful deploy.
	systemctl enable ldk-server
	if ! systemctl start ldk-server; then
		START_FAILED=1
		log "WARNING: ldk-server failed to start (check: journalctl -u ldk-server). Writing handoff anyway."
	fi
else
	# Deliberately NOT enabled: with Restart=always, an enabled unit would auto-start
	# on any reboot (manual, console, or a Linode host migration) and generate
	# keys_mnemonic unattended — the exact operator-triggered step the funds-safety
	# gate depends on. The operator arms it post-IBD with `systemctl enable --now`
	# (NEXT_STEPS step 1). Re-run safety: once the operator HAS armed it, a
	# manual re-run must not disarm it again (a later reboot would silently
	# leave the LSP down).
	if systemctl is-enabled --quiet ldk-server 2>/dev/null; then
		log "bitcoind path: ldk-server already enabled by the operator; leaving it enabled."
	else
		systemctl disable ldk-server >/dev/null 2>&1 || true
		log "bitcoind path: leaving ldk-server disabled until the operator enables it post-IBD (see NEXT_STEPS)."
	fi
fi

# --- Operator handoff (seed-free) -------------------------------------------
START_HINT="The node was started; the seed file now exists."
if [ "$CHAIN_BACKEND" = "bitcoind" ]; then
	START_HINT="Wait for bitcoind sync (bitcoin-cli -datadir=/var/lib/bitcoind getblockchaininfo => initialblockdownload=false),
then: sudo systemctl enable --now ldk-server   # this generates the seed file
    (ldk-server is deliberately left DISABLED so a reboot cannot start the
    node and generate the seed before you are ready to back it up)"
fi

cat > /root/NEXT_STEPS.txt <<EOF
================================================================================
 ldk-server LSPS2 LSP — provisioning complete. THE NODE IS UNFUNDED.
================================================================================
Network: ${NETWORK_UDF} (${LDK_NETWORK})   Backend: ${CHAIN_BACKEND}
Built commit: ${BUILD_COMMIT}

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
 4. Only then fund (mainnet: <=0.05 BTC pilot):
       sudo ldk-server-cli -c /etc/ldk-server/config.toml onchain-receive
 5. Distribute the require_token to known clients out-of-band (if set).

CLI usage: always pass the server config so the CLI finds the API key and TLS
cert (they live under /var/lib/ldk-server, readable only by the service user):
       sudo ldk-server-cli -c /etc/ldk-server/config.toml get-node-info

Check status:  systemctl status ldk-server ; journalctl -u ldk-server -f
Health check:  /opt/ldk-server-ops/health-check.sh  (units + disk; arm it via a
    cron/systemd timer once the node is enabled — see docs/linode-stackscript.md)
StackScript log: /var/log/stackscript.log
Provisioning result marker: /root/STACKSCRIPT_OK (key=value facts) or
    /root/STACKSCRIPT_FAILED.txt (reason) — exactly one exists after any run.
Upgrade later (as root; same runbook as docs/linode-stackscript.md "Upgrading").
Back up FIRST. Config keys can change between commits, so keep a rollback copy
of the working binary before overwriting it:
    cp /usr/local/bin/ldk-server /usr/local/bin/ldk-server.bak
    cd ${SRC}
    runuser -u builder -- env HOME=/opt/builder RUSTUP_HOME=/opt/rust CARGO_HOME=/opt/cargo \\
      bash -c 'git fetch && git checkout --detach <new-commit> && /opt/cargo/bin/cargo build \\
      --release --locked --features experimental-lsps2-support'
    install -m0755 target/release/ldk-server target/release/ldk-server-cli /usr/local/bin/
    systemctl restart ldk-server
If the new binary fails to start (journalctl -u ldk-server), roll back:
    install -m0755 /usr/local/bin/ldk-server.bak /usr/local/bin/ldk-server
    systemctl restart ldk-server
================================================================================
EOF
chmod 600 /root/NEXT_STEPS.txt
printf '\n*** ldk-server LSP deployed and UNFUNDED. Read /root/NEXT_STEPS.txt before funding. ***\n\n' > /etc/motd

# Honest end-state: exactly one of STACKSCRIPT_OK / STACKSCRIPT_FAILED.txt may
# exist after any run (they are the only programmatic success signal — Linode
# does not surface StackScript exit codes; contract documented in
# docs/linode-stackscript.md). Don't report success when the esplora node
# failed to start, and never leave a stale marker from an earlier run.
if [ "$START_FAILED" -eq 1 ]; then
	echo "FAILED: ldk-server did not start; see 'journalctl -u ldk-server'." > /root/STACKSCRIPT_FAILED.txt
	rm -f /root/STACKSCRIPT_OK
	log "Provisioning finished but ldk-server did NOT start — see /root/STACKSCRIPT_FAILED.txt and /root/NEXT_STEPS.txt"
	exit 1
fi
rm -f /root/STACKSCRIPT_FAILED.txt
# The OK marker carries machine-readable key=value facts (no secrets).
{
	echo "network=${NETWORK_UDF}"
	echo "chain_backend=${CHAIN_BACKEND}"
	if [ "$CHAIN_BACKEND" = "esplora" ]; then echo "node_state=started"; else echo "node_state=pending_ibd"; fi
	echo "commit=${BUILD_COMMIT}"
	echo "finished_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > /root/STACKSCRIPT_OK
log "Done. Node is UNFUNDED. See /root/NEXT_STEPS.txt"
