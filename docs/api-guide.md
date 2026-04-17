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

Every gRPC request must include an `x-auth` metadata header with an HMAC-SHA256 signature:

```
x-auth: HMAC <unix_timestamp>:<hmac_hex>
```

Where:

- `unix_timestamp` is the current time in seconds since the Unix epoch
- `hmac_hex` is the hex-encoded result of `HMAC-SHA256(api_key_bytes, timestamp_be_bytes)`
    - `api_key_bytes` is the API key string encoded as UTF-8 bytes
    - `timestamp_be_bytes` is the timestamp as a big-endian 8-byte unsigned integer

The server rejects requests where the timestamp differs from the server's clock by more than
**60 seconds**.

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
| `FAILED_PRECONDITION` (9) | Lightning operation error (e.g., insufficient balance, no route) |
| `INTERNAL` (13)           | Server-side bug                                                  |
| `UNAUTHENTICATED` (16)    | Missing or invalid `x-auth` header                               |

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

| RPC             | Description                                                       |
|-----------------|-------------------------------------------------------------------|
| `Bolt11Receive` | Create an invoice (fixed or variable amount) with automatic claim |
| `Bolt11Send`    | Pay a BOLT11 invoice (with optional routing config)               |

### BOLT11 Hodl Invoices

These RPCs support a manual claim/fail workflow for held payments. See
[Hodl Invoice Lifecycle](#hodl-invoice-lifecycle) below.

| RPC                    | Description                                                        |
|------------------------|--------------------------------------------------------------------|
| `Bolt11ReceiveForHash` | Create an invoice for a given payment hash (manual claim required) |
| `Bolt11ClaimForHash`   | Claim a held payment by providing the preimage                     |
| `Bolt11FailForHash`    | Reject a held payment                                              |

### BOLT11 JIT Channels (LSPS2)

Requires an `[liquidity.lsps2_client]` configuration. The LSP opens a channel just-in-time
when the invoice is paid.

| RPC                                        | Description                                               |
|--------------------------------------------|-----------------------------------------------------------|
| `Bolt11ReceiveViaJitChannel`               | Create a fixed-amount invoice with JIT channel opening    |
| `Bolt11ReceiveVariableAmountViaJitChannel` | Create a variable-amount invoice with JIT channel opening |

### BOLT12 Offers

| RPC             | Description                                                             |
|-----------------|-------------------------------------------------------------------------|
| `Bolt12Receive` | Create a BOLT12 offer (fixed or variable amount)                        |
| `Bolt12Send`    | Pay a BOLT12 offer (with optional quantity, payer note, routing config) |

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

Events are broadcast to all connected subscribers. The server uses a bounded broadcast channel
(capacity 1024). A slow subscriber that falls behind will miss events.

### Metrics

Metrics are served as a plain HTTP GET endpoint (not gRPC):

```
GET /metrics
```

Returns Prometheus-format text. Requires `[metrics] enabled = true` in the config. Supports
optional Basic Auth. See [Configuration](configuration.md#metrics) for setup.

## Hodl Invoice Lifecycle

Hodl invoices allow you to inspect and conditionally accept incoming payments:

1. **Create the invoice:** Call `Bolt11ReceiveForHash` with a payment hash you control.
2. **Wait for payment:** Subscribe to events via `SubscribeEvents` and watch for a
   `PaymentClaimable` event matching your payment hash.
3. **Decide:**
    - **Accept:** Call `Bolt11ClaimForHash` with the preimage corresponding to the payment hash.
    - **Reject:** Call `Bolt11FailForHash` with the payment hash.

The payment is held in a pending state until you explicitly claim or fail it. **You must
always call one of these.** If you do neither, the HTLC will eventually time out, which
can cause a force-closure of the channel.

## Pagination

`ListPayments` and `ListForwardedPayments` support cursor-based pagination:

1. Make the first request with your desired `number_of_payments` page size.
2. If the response includes a `next_page_token`, pass it as `page_token` in the next request.
3. When `next_page_token` is absent, you have reached the end of the results.

Results are ordered by creation time (most recent first).
