---
name: ldk-server-update
description: >
  Update a running ldk-server to a specific commit or tag, and install the
  matching ldk-server and ldk-server-cli together. Trigger on: update
  ldk-server, deploy ldk-server, restart ldk-server, bump ldk-server,
  install a commit, install a tag, ldk-cli auth failed after upgrade.
---

# Update ldk-server

Update one running node to one commit or tag. Always install **both**
binaries from that same build. A new server with an old CLI fails closed:

```text
Error (Authentication Error): Invalid credentials
```

The API key file did not change. The signature scheme did.

## Inputs

Ask if any of these are not already known. Do not invent them.

| Input | Example |
|---|---|
| SSH host | `user@host` |
| Revision | commit, branch, or tag |
| Build checkout | `~/src/ldk-server` |
| Config | path passed to the running process |
| Install dir | a directory on `PATH`, such as `~/.local/bin` |
| gRPC address | the address in the config, not a guessed loopback |
| TLS cert | `<storage_dir>/tls.crt` |

Read the live command line before choosing paths:

```bash
ssh HOST 'tr "\0" " " < /proc/$(pgrep -n -x ldk-server)/cmdline; echo'
```

The first argument after the binary is the config. The storage directory
is in that file. Do not assume systemd. A hand-started process reparents
to PID 1 and will not pick up a new binary on `SIGHUP`.

## Why both binaries

Since commit `819bed8` the server HMAC is:

```text
HMAC-SHA256(api_key, timestamp || raw gRPC body)
```

Older CLIs sign only the timestamp. The `x-auth` header still parses, so
the server returns `Invalid credentials`.

A current CLI hex-encodes the 32 raw bytes in `<network>/api_key` itself.
Do not pass `-a $(xxd -p api_key)` to that CLI. That double-encodes the
key and fails the same way.

Install copies. Do not symlink `ldk-cli` or `ldk-server` at
`target/release`. The next build would change one binary and leave the
other.

## Rules

- Ask before restarting. Building and installing does not require one.
- Never delete or overwrite `keys_seed`, `keys_mnemonic`, channel storage,
  `api_key`, or the TLS files.
- If both `keys_seed` and `keys_mnemonic` exist, stop. They are different
  secrets. Do not choose one.
- Do not `cargo clean` a checkout that holds the running binary.
- Do not force-push `main`.
- One SSH command per purpose.

## 1. Record the running node

```bash
ssh HOST 'set -euo pipefail
pgrep -a ldk-server
EXE=$(readlink -f /proc/$(pgrep -n -x ldk-server)/exe)
sha256sum "$EXE"
CLI=$(command -v ldk-cli || command -v ldk-server-cli)
"$CLI" -b GRPC_HOST:PORT get-node-info
'
```

Save the node id. A successful update must print the same one. If the CLI
cannot authenticate against the current server, record that and continue
with the log. Do not rotate `api_key` to make the check pass.

Also record whether `keys_seed`, `keys_mnemonic`, or both exist, and their
sizes. A 64-byte `keys_seed` is a legacy identity. Replacing it creates a
different node.

## 2. Build both binaries

Check free space on the filesystem that holds the build checkout. A
release build needs several GB.

```bash
ssh HOST 'set -euo pipefail
cd BUILD_CHECKOUT
git fetch --tags ORIGIN
git checkout --detach REV
cargo build --release -p ldk-server -p ldk-server-cli
git rev-parse HEAD
sha256sum target/release/ldk-server target/release/ldk-server-cli
'
```

`REV` must resolve before checkout. Detached HEAD is intentional. Do not
commit on the server.

If the revision is only on a fork, fetch that remote. Do not guess which
fork contains it.

## 3. Install both

Keep the previous pair for rollback, then publish the CLI first and the
server second. `mv` on the same filesystem replaces each name atomically.

```bash
ssh HOST 'set -euo pipefail
SRC=BUILD_CHECKOUT/target/release
INSTALL=INSTALL_DIR
STAGE=$(mktemp -d)
install -m 0755 "$SRC/ldk-server" "$STAGE/ldk-server"
install -m 0755 "$SRC/ldk-server-cli" "$STAGE/ldk-server-cli"
if [ -f "$INSTALL/ldk-server" ]; then
  cp -a "$INSTALL/ldk-server" "$INSTALL/ldk-server.prev"
fi
if [ -f "$INSTALL/ldk-server-cli" ]; then
  cp -a "$INSTALL/ldk-server-cli" "$INSTALL/ldk-server-cli.prev"
fi
mv -f "$STAGE/ldk-server-cli" "$INSTALL/ldk-server-cli"
mv -f "$STAGE/ldk-server" "$INSTALL/ldk-server"
ln -sfn "$INSTALL/ldk-server-cli" "$INSTALL/ldk-cli"
rmdir "$STAGE"
REV=$(git -C BUILD_CHECKOUT rev-parse --short=12 HEAD)
printf "%s %s\n" "$(date -u +%Y%m%dT%H%M%SZ)" "$REV" > "$INSTALL/ldk-server.rev"
cmp "$INSTALL/ldk-server" "$SRC/ldk-server"
cmp "$INSTALL/ldk-server-cli" "$SRC/ldk-server-cli"
'
```

Stop here unless restart was requested. The old process keeps running the
previous server. A new CLI may already fail against it if the auth scheme
changed. Say that. Do not restart to make the CLI work unless asked.

## 4. Restart only when asked

`SIGHUP` reopens the log. It does not exec a new binary.

If a systemd unit actually supervises the process, restart that unit and
skip the manual launch. Confirm with `systemctl status`, not by assuming
the sample unit in `contrib/ldk-server.service` is installed.

Otherwise:

```bash
ssh HOST 'set -euo pipefail
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
nohup INSTALL_DIR/ldk-server CONFIG \
  >> STORAGE_DIR/ldk-server.stdout 2>&1 &
echo $! > STORAGE_DIR/ldk-server.pid
disown || true
'
```

Wait until the log says the gRPC listener is bound before calling the CLI.
If it does not come up, do not start a second copy.

## 5. Prove it

```bash
ssh HOST 'set -euo pipefail
pgrep -a ldk-server
readlink -f /proc/$(pgrep -n -x ldk-server)/exe
sha256sum /proc/$(pgrep -n -x ldk-server)/exe INSTALL_DIR/ldk-server
INSTALL_DIR/ldk-cli -b GRPC_HOST:PORT get-node-info
INSTALL_DIR/ldk-cli -b GRPC_HOST:PORT list-channels
'
```

Pass only if all of these are true:

- The process executable is the installed `ldk-server`, not a leftover
  checkout binary.
- Its sha256 matches the installed file.
- `ldk-cli` resolves to the CLI installed in the same step.
- `get-node-info` returns the node id recorded in step 1.
- `keys_seed` and `keys_mnemonic` were not replaced. A new
  `keys_mnemonic` next to an old `keys_seed` means the process generated
  a second identity. Stop and report it.
- `list-channels` still shows the channels that were usable before.

The log line `Starting ldk-server version` includes the git commit. Match
it to `ldk-server.rev`.

## Rollback

Move `ldk-server.prev` and `ldk-server-cli.prev` back into place, refresh
the `ldk-cli` symlink, and restart only if asked. Never roll one binary
back and leave the other.

Channel storage is not part of the binary rollback. Do not restore an
older sqlite or postgres snapshot over the one the node just wrote.

## Auth failure after an update

1. Confirm `ldk-cli` is the CLI from the same install as the server.
2. Compare mtimes and the rev file. They must name one commit.
3. Do not hex-encode `api_key` again.
4. If the CLI is older than the server, reinstall the pair from the
   running commit. Do not rotate `api_key`.
