# ldk-server-grpc

Canonical Protocol Buffer definitions for the [LDK Server](https://github.com/lightningdevkit/ldk-server)
API, along with generated Rust types and shared gRPC primitives.

This crate has **no LDK dependency** and can be used by anyone who wants to speak the LDK
Server wire protocol.

## Proto Files

The proto definitions live in `src/proto/`:

| File | Contents |
|------|----------|
| `api.proto` | RPC request/response messages with documentation |
| `types.proto` | Shared types: Payment, Channel, Peer, ForwardedPayment, etc. |
| `events.proto` | Event envelope and event types for streaming |
| `error.proto` | Error response definitions |

## Using from Other Languages

The proto files can be compiled with any standard `protoc` toolchain to generate clients in
Go, Python, TypeScript, Java, etc. Point your proto compiler at the `src/proto/` directory.

## Regenerating Rust Bindings

After modifying any `.proto` file:

```bash
RUSTFLAGS="--cfg genproto" cargo build -p ldk-server-grpc
cargo fmt --all
```

This requires `protoc` to be installed.

## Features

- **`serde`**: Enables `serde::Serialize` and `serde::Deserialize` on all generated types,
  with custom serialization for hex-encoded fields.

## Additional Rust Modules

Beyond the generated types, this crate provides:

- `grpc`: gRPC frame encoding/decoding, error response helpers, request validation
- `endpoints`: Path constants for all RPC methods (e.g., `GRPC_SERVICE_PREFIX`, `GET_NODE_INFO_PATH`)
