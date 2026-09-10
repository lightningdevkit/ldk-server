# API Guide

LDK Server exposes a gRPC API over HTTP/2 with TLS. This guide covers transport, authentication,
and provides an index of all available RPCs. For field-level details on each request and response,
refer to the proto definitions, which are the canonical reference and include links to the
underlying LDK Node documentation.

## Transport

- **Protocol:** gRPC over HTTP/2 with TLS (self-signed by default)
- **Default address:** `127.0.0.1:3536`
- **Content-Type:** `application/grpc+proto`
- **Service name:** `api.LightningNode`
- **Full RPC path format:** `/api.LightningNode/<MethodName>`

## Authentication

Each request needs a hex-encoded v2 macaroon in the `macaroon` header:

```text
macaroon: <hex-encoded-request-macaroon>
```

Keep your original macaroon private. The client uses it to make a token for each request,
as described below. All requests use TLS.

You can add caveats to make a restricted copy of your macaroon without contacting the server.
A caveat limits its permissions, allowed methods, or expiry time. Added caveats can only reduce
access. See [Restrictions](#restrictions) for examples.

### Request binding

The Rust client, CLI, and MCP handle request binding automatically. They keep your macaroon
private and send a copy tied to the request's method, body, and time.

Custom clients must add one final caveat:

```text
request = <unix-seconds> <RpcMethod> <body-sha256>
```

Use Unix time in seconds, a method name such as `OnchainSend`, and the lowercase SHA-256 hash
of the exact gRPC body, including its five-byte frame header. Rust clients can use
`macaroon::bind_macaroon_to_request`.

Client and server clocks must be within 60 seconds. The token cannot authorize a different
request, but the same request can still be replayed while the token is valid.

See [Request proof format](request-binding.md) for exact encoding rules and an example.

### Restrictions

A caveat is a condition that limits what a macaroon can do. All caveats must pass:

| Caveat | Meaning |
|--------|---------|
| `permissions = node:read,payments:read` | Allow only these permissions |
| `method = GetNodeInfo` | Allow only this RPC method |
| `time-before = 1800000000` | Expire at this Unix time in seconds |

Added caveats can only reduce access. They cannot restore permissions or extend the expiry time.

Derive a restricted copy without contacting the server:

```bash
ldk-server-cli derive-macaroon "$MACAROON" \
  --caveat 'permissions = node:read' \
  --caveat 'method = GetNodeInfo' \
  --caveat "time-before = $EXPIRY_UNIX_SECONDS"
```

The command prints a hex token. Rust clients can use `macaroon::derive_macaroon`.
Give the copy to the application and keep the original private.

### Create and revoke tokens

Use `CreateMacaroon` to give each client a token you can revoke separately. New tokens keep
all the caller's restrictions. Revoking the caller's token does not revoke these new tokens.

Copies made with `derive-macaroon` share the original token's ID. Revoking that ID blocks
all those copies. The server cannot list copies made locally.

Revocation and expiry block new requests. Existing event streams stay open until the client
disconnects or the server stops. Reconnecting requires a valid token.

See [Macaroon Management](#macaroon-management) for the RPCs and
[Operations](operations.md#macaroons) for storage and recovery.

### Macaroon Permissions

Choose the permissions each client needs, or use `admin` by itself for full access.
The CLI also has `readonly`, `invoice`, and `admin` presets.
RPCs with no permission mapping return `UNIMPLEMENTED`, even for admin tokens.

| Permission | Access |
| ---------- | ------ |
| `node:read` | Node information, balances, and pathfinding scores |
| `onchain:receive` | Create on-chain receive addresses |
| `onchain:send` | Send on-chain funds |
| `invoices:create` | Create BOLT11/BOLT12 invoices and incoming refund requests |
| `payments:read` | Read payments and forwarded payments |
| `payments:claim` | Claim or fail held BOLT11 payments |
| `payments:send` | Send BOLT11, BOLT12, spontaneous, unified, and refund payments |
| `channels:read` | List channels |
| `channels:manage` | Open, configure, or cooperatively close channels |
| `channels:splice` | Splice funds in or out, including to an external address |
| `channels:force_close` | Force-close channels |
| `peers:read` | List peers |
| `peers:manage` | Connect or disconnect peers |
| `messages:sign` | Sign messages and create BOLT12 payer proofs |
| `messages:verify` | Verify message signatures |
| `graph:read` | Read network graph data |
| `utilities:read` | Decode invoices and offers |
| `events:read` | Subscribe to the event stream |
| `macaroons:manage` | Create, list, and revoke macaroons within your permissions |

MCP provides token management through `create_macaroon`, `list_macaroons`, `revoke_macaroon`,
and `get_permissions`.

## TLS

The server auto-generates a self-signed ECDSA P-256 certificate on first startup, stored at
`<storage_dir>/tls.crt`. Clients must pin this certificate (not rely on system trust roots)
since it is self-signed.

For the Rust client library, pass the PEM contents to `LdkServerClient::new()`. For other
languages, configure your gRPC channel to trust the server's certificate file.

## Proto Definitions

The canonical API definitions live in `ldk-server-grpc/src/proto/`:

| File           | Contents                                            |
|----------------|-----------------------------------------------------|
| `api.proto`    | All RPC request/response messages and documentation |
| `types.proto`  | Shared types (Payment, Channel, Peer, etc.)         |
| `events.proto` | Event envelope and event types for streaming        |
| `error.proto`  | Error response definitions                          |

### Generating Client Stubs

Any standard `protoc` toolchain can generate clients from these proto files. The proto directory
path is `ldk-server-grpc/src/proto/`. For Rust specifically, the `ldk-server-client` crate
provides a ready-made async client.

## Error Model

Errors are returned as standard gRPC status codes:

| gRPC Code                 | Meaning                                                          |
|---------------------------|------------------------------------------------------------------|
| `INVALID_ARGUMENT` (3)    | Malformed request or invalid parameters                          |
| `PERMISSION_DENIED` (7)   | Missing permission or a caveat that does not pass                    |
| `FAILED_PRECONDITION` (9) | Lightning operation error (e.g., insufficient balance, no route) |
| `INTERNAL` (13)           | Server-side bug                                                  |
| `UNAUTHENTICATED` (16)    | Missing, invalid, or revoked macaroon                               |

The `grpc-message` trailer contains a human-readable error description.

## Endpoint Reference

All RPCs are unary (single request, single response) unless noted otherwise.

### Node Information

| RPC           | Description                                                                         |
|---------------|-------------------------------------------------------------------------------------|
| `GetNodeInfo` | Node ID, best block, sync timestamps, listening/announcement addresses, alias, URIs |
| `GetBalances` | On-chain, Lightning channel, and claimable balance breakdown                        |

### On-Chain

| RPC              | Description                                                          |
|------------------|----------------------------------------------------------------------|
| `OnchainReceive` | Generate a new on-chain funding address                              |
| `OnchainSend`    | Send to a Bitcoin address (with optional fee rate and send-all mode) |

### BOLT11 Payments

| RPC                     | Description                                                       |
|-------------------------|-------------------------------------------------------------------|
| `Bolt11Receive`         | Create an invoice (fixed or variable amount) with automatic claim |
| `Bolt11Send`            | Pay a BOLT11 invoice (with optional routing config)               |
| `Bolt11SendUnderpaying` | Send part of the amount for a fixed-amount BOLT11 invoice         |

> [!NOTE]
> `Bolt11SendUnderpaying` sends one part of a multi-part payment (MPP) for a fixed-amount
> BOLT11 invoice. Other nodes must send compatible partial payments for the same invoice
> until the combined amount equals the invoice amount. Without those payments, the
> receiver holds the incomplete MPP payment and eventually fails it.

### BOLT11 Hodl Invoices

These RPCs support a manual claim/fail workflow for held payments. See
[Hodl Invoice Lifecycle](#hodl-invoice-lifecycle) below.

| RPC                    | Description                                                        |
|------------------------|--------------------------------------------------------------------|
| `Bolt11ReceiveForHash` | Create an invoice for a given payment hash (manual claim required) |
| `Bolt11ClaimForId`      | Claim a held payment by its payment ID and preimage                |
| `Bolt11FailForId`       | Reject a held payment by its payment ID                            |

### BOLT11 JIT Channels (LSPS2)

Requires at least one `[[liquidity.lsps_client]]` entry for an LSPS2-capable LSP. The LSP opens
a channel just-in-time when the invoice is paid.

| RPC                                        | Description                                               |
|--------------------------------------------|-----------------------------------------------------------|
| `Bolt11ReceiveViaJitChannel`               | Create a fixed-amount invoice with JIT channel opening    |
| `Bolt11ReceiveVariableAmountViaJitChannel` | Create a variable-amount invoice with JIT channel opening |

### BOLT12 Offers and Refunds

| RPC                      | Description                                                             |
|--------------------------|-------------------------------------------------------------------------|
| `Bolt12Receive`          | Create a BOLT12 offer (fixed or variable amount)                        |
| `Bolt12Send`             | Pay a BOLT12 offer (with optional quantity, payer note, routing config) |
| `Bolt12SendRefund`       | Create a BOLT12 refund that this node will pay                          |
| `Bolt12ReceiveRefund`    | Request an incoming payment for a BOLT12 refund                         |
| `Bolt12CreatePayerProof` | Create a BOLT 12 payer proof from a successful payment                  |

### Spontaneous and Unified Send

| RPC               | Description                                                                    |
|-------------------|--------------------------------------------------------------------------------|
| `SpontaneousSend` | Send a keysend payment to a node ID                                            |
| `UnifiedSend`     | Pay a BIP 21 URI, BIP 353 Human-Readable Name, BOLT11 invoice, or BOLT12 offer |

### Channel Management

| RPC                   | Description                                                            |
|-----------------------|------------------------------------------------------------------------|
| `OpenChannel`         | Open a new outbound channel (with optional push amount and fee config) |
| `CloseChannel`        | Cooperatively close a channel                                          |
| `ForceCloseChannel`   | Force-close a channel unilaterally                                     |
| `SpliceIn`            | Add on-chain funds to an existing channel                              |
| `SpliceOut`           | Remove funds from a channel back on-chain                              |
| `UpdateChannelConfig` | Update forwarding fees and CLTV expiry delta                           |
| `ListChannels`        | List all channels with balances and configuration                      |

### Payment History

| RPC                     | Description                                    |
|-------------------------|------------------------------------------------|
| `GetPaymentDetails`     | Get details for a specific payment by ID       |
| `ListPayments`          | List all payments (paginated)                  |
| `ListForwardedPayments` | List all forwarded/routed payments (paginated) |

See [Pagination](#pagination) below for how to page through results.

### Peer Management

| RPC              | Description                                              |
|------------------|----------------------------------------------------------|
| `ConnectPeer`    | Connect to a peer (optionally persist the connection)    |
| `DisconnectPeer` | Disconnect from a peer and remove it from the peer store |
| `ListPeers`      | List all connected peers                                 |

### Cryptography

| RPC               | Description                                         |
|-------------------|-----------------------------------------------------|
| `SignMessage`     | Sign a message with the node's private key          |
| `VerifySignature` | Verify a signature against a message and public key |

### Network Graph

| RPC                 | Description                                           |
|---------------------|-------------------------------------------------------|
| `GraphListChannels` | List all known short channel IDs in the network graph |
| `GraphGetChannel`   | Get channel details by short channel ID               |
| `GraphListNodes`    | List all known node IDs in the network graph          |
| `GraphGetNode`      | Get node details by node ID                           |

### Routing

| RPC                       | Description                                          |
|---------------------------|------------------------------------------------------|
| `ExportPathfindingScores` | Export the router's pathfinding score cache          |
| `DecodeInvoice`           | Decode a BOLT11 invoice and return its parsed fields |
| `DecodeOffer`             | Decode a BOLT12 offer and return its parsed fields   |

### Event Streaming

| RPC               | Description                                                 |
|-------------------|-------------------------------------------------------------|
| `SubscribeEvents` | **Server-streaming.** Subscribe to real-time payment and channel events |

`SubscribeEvents` returns a stream of `EventEnvelope` messages. Each envelope contains one of:

| Event               | When                                                                  |
|---------------------|-----------------------------------------------------------------------|
| `PaymentReceived`   | An inbound payment was received and auto-claimed                      |
| `PaymentSuccessful` | An outbound payment succeeded                                         |
| `PaymentFailed`     | An outbound payment failed                                            |
| `PaymentClaimable`  | A hodl invoice payment arrived and is waiting to be claimed or failed |
| `PaymentForwarded`  | A payment was routed through this node                                |
| `ChannelStateChanged` | A channel changed state (pending, ready, open failed, closed)      |
| `SpliceNegotiated` | A channel splice was negotiated and the funding transaction is pending confirmation |
| `SpliceNegotiationFailed` | A channel splice negotiation round failed                       |

> [!WARNING]
> `SubscribeEvents` is a best-effort stream of new events. Events are not persisted for
> subscribers, cannot be replayed after reconnecting, and have no client acknowledgement.
> Acceptance by the server's broadcast channel does not guarantee that a client received or
> processed an event.

Events are broadcast to all currently connected subscribers. The server uses a bounded broadcast
channel (capacity 1024), so a slow subscriber that falls behind will miss events. Disconnected
clients also miss events and receive only new events after reconnecting. If the server cannot read
data required to construct a payment event, it logs the error and skips that event so the event
queue can continue processing.

Use events as notifications. After reconnecting, reconcile recoverable state with APIs such as
`GetPaymentDetails`, `ListPayments`, `ListForwardedPayments`, and `ListChannels`. Some event fields
cannot be recovered through these APIs.

### Macaroon Management

| RPC | Description |
|-----|-------------|
| `CreateMacaroon` | Create a macaroon and return its private hex token in `token` |
| `ListMacaroons` | List IDs, names, permissions, and caveats, without secrets |
| `RevokeMacaroon` | Revoke a macaroon by ID |
| `GetPermissions` | Show the caller's ID, name, usable permissions, and caveats |

The first three RPCs require `macaroons:manage` or `admin`. You can only create or revoke
tokens whose permissions you have. The last unrestricted admin token cannot be revoked.

### Metrics

Metrics are served as a plain HTTP GET endpoint (not gRPC):

```
GET /metrics
```

Returns Prometheus-format text. Requires `[metrics] enabled = true` in the config. Supports
optional Basic Auth. See [Configuration](configuration.md#metrics) for setup.

## BOLT 12 Payer-Proof Lifecycle

Subscribe with `SubscribeEvents` before you send a BOLT 12 payment. Events are not replayed.

When `PaymentSuccessful` arrives, retain its `payment_id`, `payment_preimage`, and
`bolt12_invoice`. Pass these values to `Bolt12CreatePayerProof`. The request can also select the
optional invoice fields that the proof discloses. Payment history APIs cannot recover all the
inputs required to create a proof if this event is missed. Save these values before processing
other events.

The `bolt12_invoice` field is absent for static-invoice payments. These asynchronous payments
cannot produce payer proofs.

## Hodl Invoice Lifecycle

Hodl invoices allow you to inspect and conditionally accept incoming payments:

1. **Subscribe:** Call `SubscribeEvents` before you create or share the invoice. Events are not
   replayed.
2. **Create the invoice:** Generate a new payment hash. Call `Bolt11ReceiveForHash` with this hash.
   Never reuse a payment hash. Reuse is unsafe and can cause loss of funds.
3. **Handle each payment:** Save the payment ID from each `PaymentClaimable` event. A payer can pay
   the same invoice more than once. Each payment has a separate event and payment ID.
4. **Decide before `claim_deadline`:**
    - **Accept an expected payment:** Check the event's `claimable_amount_msat` against the amount
      you expect. Call `Bolt11ClaimForId` with its payment ID, preimage, and the event's claimable
      amount.
    - **Reject an unexpected payment:** Call `Bolt11FailForId` with its payment ID. Reject duplicate
      and late payments instead of ignoring or claiming them.

The claim request's optional amount is passed to LDK Node for a lower-bound check against its
stored payment amount, less any skimmed fee. It is not an exact amount check or a request to claim
that many millisatoshis. A larger supplied amount passes this check; omitting it skips the check.
Always validate the event's amount before you claim the payment.

The payment is held in a pending state until you claim it, fail it, or its `claim_deadline` is
reached. `PaymentClaimable` notifications are best-effort and are not replayed. If you miss the
event or do not act before the deadline, LDK Node automatically fails the HTLC backward and the
payment can no longer be claimed. Keep the subscriber healthy and resolve reported persistence
errors before accepting further payments.

## Pagination

`ListPayments` and `ListForwardedPayments` support cursor-based pagination:

1. Make the first request without a `page_token`. The server controls the page size.
2. If the response includes a `next_page_token`, pass it as `page_token` in the next request.
3. When `next_page_token` is absent, you have reached the end of the results.

The page token is one opaque string. Do not parse or modify it. Results are ordered by creation
time (most recent first).

The CLI `--number-of-payments` option combines multiple pages. It does not set the gRPC page size.
