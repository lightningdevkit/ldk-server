# Running with Pruned Bitcoin Core

LDK Server supports a pruned Bitcoin Core node through the `[bitcoind]` RPC backend. Pruning
removes old block and undo data while retaining the headers and UTXO set. Routine operation only
needs blocks after LDK Server's persisted chain tip, but a new wallet rescan or a stale node may
need blocks that Core has already deleted.

## Configure Bitcoin Core

Choose a prune target large enough to cover the longest expected LDK Server outage. Bitcoin Core
accepts automatic targets of at least 550 MiB and always keeps at least 288 recent blocks, but a
larger target provides a safer catch-up window. This example allocates roughly 10 GiB:

```ini
chain=main
server=1
prune=10000
rpcbind=127.0.0.1
rpcallowip=127.0.0.1
rpcuser=ldkserver
rpcpassword=<long-random-password>
```

Store `bitcoin.conf` with restrictive permissions. Do not expose RPC to the public Internet. LDK
Server requires an RPC username and password; it does not read Core's cookie file. For hashed Core
configuration, generate an `rpcauth` entry with Bitcoin Core's `share/rpcauth/rpcauth.py` and give
LDK Server the corresponding cleartext password.

Start Core, wait for it to synchronize, and inspect its state:

```bash
bitcoin-cli -chain=main getblockchaininfo
```

Confirm that `initialblockdownload` is `false`, `pruned` is `true`, and note `pruneheight`.
`pruneheight` is the first height whose block data remains locally available; it may remain `0`
until Core actually deletes a block file.

## Configure LDK Server

Use Core's mainnet RPC port, which defaults to `8332`, not its peer-to-peer port (`8333`). Match
LDK Server's `bitcoin` network to Core's `main` chain:

```toml
[node]
network = "bitcoin"

[storage.disk]
dir_path = "/var/lib/ldk-server"

[bitcoind]
rpc_address = "127.0.0.1:8332"
rpc_user = "ldkserver"
rpc_password = "<long-random-password>"
```

On a fresh start without `--rescan-from-height`, LDK Server checkpoints its wallet at Core's
current tip. This is appropriate for a new node whose addresses have never received funds. Watch
startup logs for `Finished synchronizing listeners`; the gRPC listener can become available before
initial chain synchronization finishes.

## Retention and Routine Operation

Keep Bitcoin Core reachable while LDK Server is running. Stop LDK Server gracefully before Core
maintenance so its latest chain state is persisted. After startup, compare
`get-node-info.current_best_block.height` with Core's `blocks` value from `getblockchaininfo`.

If LDK Server is offline while Core prunes past its persisted tip, catch-up will fail with
`Block not available (pruned data)`. A larger prune target reduces this risk but does not express a
fixed time window because block sizes vary. Restore access to the missing block range using an
archival Core node before resuming normal operation; after LDK Server catches up and shuts down
cleanly, it can use the pruned backend again.

## Channel Operation

Once initial chain synchronization completes, pruning does not change the channel workflow or its
confirmation requirements. Funding transactions, Lightning payments, and cooperative or force
closes operate normally because LDK Server watches new blocks as Core receives them. Before
opening or using channels, verify that `current_best_block.height` tracks Core's tip; API
availability alone does not prove that chain listeners are current.

The persisted-tip retention rule becomes especially important after channels exist. If LDK Server
cannot obtain every block since its last persisted tip, do not resume routine channel operations
while it is retrying synchronization. Restore the missing history from an archival backend, allow
listener synchronization to finish, and only then resume normal operation.

## Wallet Rescans and Recovery

Use `--rescan-from-height` when a fresh wallet must discover transactions created before its first
startup, for example during mnemonic recovery. Replace the value below with a mainnet height at or
before the earliest relevant wallet transaction:

```bash
rescan_height=REPLACE_WITH_EARLIEST_RELEVANT_MAINNET_HEIGHT
```

The option only initializes a new wallet; it does not rewind existing persisted wallet state.
Before starting, require:

```text
rescan height >= getblockchaininfo.pruneheight
```

You can also verify the boundary directly:

```bash
bitcoin-cli -chain=main getblock \
  "$(bitcoin-cli -chain=main getblockhash "$rescan_height")" 0 >/dev/null
```

If that succeeds, start the initial scan:

```bash
ldk-server /etc/ldk-server/config.toml --rescan-from-height "$rescan_height"
```

If the requested range is pruned, LDK Server may expose gRPC while chain synchronization logs
repeated transient errors. Stop it and recover against an archival backend (or re-download Core's
chain with pruning disabled). Retry from clean recovery state derived from verified backups; a
failed first scan may already have persisted its starting checkpoint.

The mnemonic recovers on-chain keys only. Lightning channel recovery also requires the current
`ldk_node_data.sqlite`; follow the [backup guidance](operations.md#backups), and never run two
instances from the same node identity.
