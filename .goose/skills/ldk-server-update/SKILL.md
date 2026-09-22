---
name: ldk-server-update
description: >
  Update the production ldk-server on vincent@65.108.246.14 to a specific
  commit or tag, and install the matching ldk-server and ldk-server-cli
  together. Trigger on: update ldk-server, deploy ldk-server, restart
  ldk-server, bump ldk-server, install a commit, install a tag,
  ldk-cli auth failed after upgrade.
---

# Update production ldk-server

Update the live node to one commit or tag. Always install **both**
binaries from that same build. A new server with the old CLI fails
closed: `Error (Authentication Error): Invalid credentials`.

This is the Hetzner node, not a generic install. Do not invent a
systemd unit. The process is started by hand and reparented to PID 1.

## Host

| Item | Value |
|---|---|
| SSH | `vincent@65.108.246.14` |
| Config | `/home/vincent/ldk-server-mainnet.toml` |
| Storage | `/home/vincent/.ldk-server/` |
| Secrets | `keys_seed` or `keys_mnemonic`, `bitcoin/api_key`, `tls.crt`, `tls.key` |
| gRPC | `65.108.246.14:3536` (TLS, not loopback) |
| P2P | `127.0.0.1:9735` |
| Build checkout | `/home/vincent/src/ldk-server` |
| Install dir | `/home/vincent/.local/bin` |
| Commands | `ldk-server`, `ldk-server-cli`, `ldk-cli` → `ldk-server-cli` |
| Rev file | `/home/vincent/.local/bin/ldk-server.rev` |
| Fork remote | `git@github.com:vincenzopalazzo/ldk-server.git` |
| Upstream | `git@github.com:lightningdevkit/ldk-server.git` |
| Root disk | `/` — build here. `/mnt/HC_Volume_103194752` stays near full; do not build there. |

`~/.local/bin` is already on `PATH`.

## Why both binaries

Since `819bed8` the server HMAC is `HMAC-SHA256(api_key, timestamp || body)`.
The July 2026 CLI signs only the timestamp. The header still parses, so
the only error is `Invalid credentials`. The `api_key` file did not change.

The CLI hex-encodes the 32 raw bytes in `bitcoin/api_key` itself. Do not
pass `-a $(xxd -p api_key)` to a current CLI. That double-encodes the key
and fails the same way.

Never leave `ldk-cli` pointing at a checkout under
`/mnt/HC_Volume_103194752`. That symlink is how the July CLI survived the
last upgrade.

## Rules

- Ask before restarting. Building and installing does not require a restart.
- Never delete or overwrite `keys_seed`, `keys_mnemonic`, channel storage,
  `api_key`, or the TLS files.
- If both `keys_seed` and `keys_mnemonic` exist, stop. They are different
  secrets. Do not choose one.
- Do not `cargo clean` on the volume checkout.
- Do not force-push `main`.
- One SSH command per purpose. A `====` banner breaks the remote zsh.

## 1. Resolve the revision

`<rev>` is a commit, branch, or tag. Prefer the fork if the commit is
only there (`feat/keys-seed-compat` and similar). Use upstream for a
stock release.

```bash
ssh vincent@65.108.246.14 'set -euo pipefail
cd /home/vincent/src/ldk-server
git fetch origin
git fetch https://github.com/lightningdevkit/ldk-server.git main:refs/remotes/upstream/main
git rev-parse --verify "<rev>^{commit}"
git log -1 --oneline "<rev>"
'
```

If `<rev>` is not on the fork, fetch the URL or ref that contains it.
Do not guess.

Record the running identity before changing anything:

```bash
ssh vincent@65.108.246.14 'set -euo pipefail
pgrep -a ldk-server
tr "\0" " " < /proc/$(pgrep -n -x ldk-server)/cmdline; echo
sha256sum /proc/$(pgrep -n -x ldk-server)/exe
/home/vincent/.local/bin/ldk-cli -b 65.108.246.14:3536 get-node-info
ls -la /home/vincent/.ldk-server/keys_seed /home/vincent/.ldk-server/keys_mnemonic /home/vincent/.ldk-server/bitcoin/api_key
'
```

Save the node id. A successful update must print the same one.

## 2. Build both binaries

Build in `/home/vincent/src/ldk-server`, on `/`. Check `df -h /` first.
Need several GB free. `rustc` on this host is 1.95, which is enough for
current `main`.

```bash
ssh vincent@65.108.246.14 'set -euo pipefail
cd /home/vincent/src/ldk-server
git fetch origin
git checkout --detach "<rev>"
cargo build --release -p ldk-server -p ldk-server-cli
git rev-parse HEAD
sha256sum target/release/ldk-server target/release/ldk-server-cli
'
```

Detached HEAD is intentional. Do not commit on the server. Do not switch
the operator laptop checkout to do this.

## 3. Install both, atomically

Copy the binaries. Do not symlink them back into `target/release`. A
later build would change the CLI under a running server, or the reverse.

```bash
ssh vincent@65.108.246.14 'set -euo pipefail
SRC=/home/vincent/src/ldk-server/target/release
INSTALL=/home/vincent/.local/bin
STAGE=$(mktemp -d)
install -m 0755 "$SRC/ldk-server" "$STAGE/ldk-server"
install -m 0755 "$SRC/ldk-server-cli" "$STAGE/ldk-server-cli"
# publish cli first, then the server binary, then the ldk-cli name
mv -f "$STAGE/ldk-server-cli" "$INSTALL/ldk-server-cli"
mv -f "$STAGE/ldk-server" "$INSTALL/ldk-server"
ln -sfn "$INSTALL/ldk-server-cli" "$INSTALL/ldk-cli"
rmdir "$STAGE"
REV=$(git -C /home/vincent/src/ldk-server rev-parse --short=12 HEAD)
printf "%s %s\n" "$(date -u +%Y%m%dT%H%M%SZ)" "$REV" > "$INSTALL/ldk-server.rev"
sha256sum "$INSTALL/ldk-server" "$INSTALL/ldk-server-cli"
cmp "$INSTALL/ldk-server" "$SRC/ldk-server"
cmp "$INSTALL/ldk-server-cli" "$SRC/ldk-server-cli"
'
```

Stop here unless the operator asked to restart. The old process keeps
running the previous server binary. The new CLI may already fail against
it if the auth scheme changed. Say that, and do not "fix" it by restarting
unasked.

## 4. Restart only when asked

There is no systemd unit. The live process has PPID 1. `SIGHUP` reopens
the log. It does **not** exec a new binary.

```bash
ssh vincent@65.108.246.14 'set -euo pipefail
OLD=$(pgrep -n -x ldk-server)
kill -TERM "$OLD"
for _ in 1 2 3 4 5 6 7 8 9 10; do
  kill -0 "$OLD" 2>/dev/null || break
  sleep 1
done
if kill -0 "$OLD" 2>/dev/null; then
  echo "old process still alive; not sending KILL" >&2
  exit 1
fi
nohup /home/vincent/.local/bin/ldk-server /home/vincent/ldk-server-mainnet.toml \
  >> /home/vincent/.ldk-server/ldk-server.stdout 2>&1 &
echo $! > /home/vincent/.ldk-server/ldk-server.pid
disown || true
'
```

Wait until the log says the gRPC listener is up before calling the CLI.
If it does not come up, do not start a second copy.

## 5. Prove it

```bash
ssh vincent@65.108.246.14 'set -euo pipefail
pgrep -a ldk-server
tr "\0" " " < /proc/$(pgrep -n -x ldk-server)/cmdline; echo
readlink -f /proc/$(pgrep -n -x ldk-server)/exe
sha256sum /proc/$(pgrep -n -x ldk-server)/exe /home/vincent/.local/bin/ldk-server
/home/vincent/.local/bin/ldk-cli -b 65.108.246.14:3536 get-node-info
/home/vincent/.local/bin/ldk-cli -b 65.108.246.14:3536 list-channels
'
```

Pass only if all of these are true:

- The process exe is `/home/vincent/.local/bin/ldk-server`.
- Its sha256 matches `ldk-server-cli`'s sibling from the same install.
- `ldk-cli get-node-info` returns the node id recorded in step 1.
- `keys_seed` / `keys_mnemonic` were not replaced. A new `keys_mnemonic`
  next to an old `keys_seed` means the process generated a second identity.
  Stop and report it.
- `list-channels` still shows the channels that were usable before.

Also check the log for `Loaded node entropy from` and
`Starting ldk-server version`. The version line includes the git commit.

## Rollback

Keep the previous pair. Before installing, if those files exist:

```bash
cp -a ~/.local/bin/ldk-server ~/.local/bin/ldk-server.prev
cp -a ~/.local/bin/ldk-server-cli ~/.local/bin/ldk-server-cli.prev
```

Rollback is the same restart, with the `.prev` files moved back into
place, then `ln -sfn` for `ldk-cli`. Never roll the server back and leave
the new CLI, or the reverse.

Channel storage is not part of the binary rollback. Do not restore an
older sqlite/postgres snapshot over the one the node just wrote.

## Auth failure after an update

If `get-node-info` returns `Invalid credentials`:

1. Confirm `ldk-cli` resolves to `~/.local/bin/ldk-server-cli`.
2. Confirm that file's mtime matches `~/.local/bin/ldk-server`.
3. Do not hex-encode `api_key` again.
4. If the CLI is older than the server, reinstall the pair from the
   running commit. Do not rotate `api_key`.
