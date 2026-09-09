# Request proof format

This document gives the wire format for custom clients. The Rust client, CLI, and MCP
create request proofs automatically. See the [API guide](api-guide.md#authentication)
for normal use and supported policy caveats.

## Create a request token

1. Start with a private, reusable v2 macaroon. It must not contain a `request = ` caveat.
2. Encode the protobuf request and its gRPC frame header. Hash these exact bytes with SHA-256.
3. Add the proof below as the final first-party caveat. Use standard macaroon signing:
   `new_signature = HMAC-SHA256(previous_signature, proof_bytes)`. Both signatures are raw
   32-byte values; `proof_bytes` are the ASCII caveat text.
4. Serialize the resulting v2 macaroon and hex-encode it. Send it in the `macaroon` metadata
   header over TLS, with no prefix. Keep the reusable macaroon private.

Do not edit or replace the proof in a request token. Make each request token from the reusable
macaroon. Rust code can use `ldk_server_macaroons::bind_macaroon_to_request`.

## Caveat grammar

```text
request = <unix-seconds> <RpcMethod> <body-sha256>
```

The text is case-sensitive. Each space shown is exactly one ASCII space. There must be no
leading or trailing whitespace, newline, or extra field.

| Field | Encoding |
|-------|----------|
| `unix-seconds` | Unix time in whole seconds, from `0` through `18446744073709551615`. Decimal digits only; no sign or leading zeros, except `0` itself. |
| `RpcMethod` | The short RPC name, such as `GetNodeInfo`. From 1 to 128 ASCII letters or digits (`A-Z`, `a-z`, `0-9`). Do not include the service name or URL path. |
| `body-sha256` | Exactly 64 lowercase hex characters (`0-9`, `a-f`), representing the SHA-256 hash of the exact gRPC body sent. |

There must be exactly one request proof, in the final caveat position. All policy caveats go
before it. The proof is not a reusable restriction and is not inherited by newly issued
credentials or returned by `GetPermissions`.

## Body bytes

Hash the five-byte gRPC frame header followed by the protobuf bytes:

```text
00 || protobuf_length_as_4_byte_big_endian_integer || protobuf_bytes
```

The first byte is the compression flag. The server accepts only uncompressed requests.
Do not hash HTTP headers, HTTP/2 frame headers, or a JSON form of the request. Do not encode
the protobuf again after hashing; send the same bytes. The server limits the request body,
including the gRPC frame header, to 10 MiB.

An empty `GetNodeInfo` request has five zero bytes. At Unix time `1800000000`, its proof is:

```text
request = 1800000000 GetNodeInfo 8855508aade16ec573d21e6a485dfd0a7624085c1a14b5ecdd6485de0c6839a4
```

The final row in the [reference vectors](../ldk-server-macaroons/tests/data/macaroons-v2.txt)
contains this proof and its signed token. Its fixed timestamp is for format checks.

## Limits and checks

- A token can contain at most 4096 binary bytes (8192 hex characters) and 32 caveats,
  including the proof. Transport hex can use either case; the body hash inside the proof
  must be lowercase.
- Reusable credentials must reserve one caveat slot and 228 bytes for the largest proof:
  224 bytes of text plus four bytes of v2 encoding. The Rust helpers check this reserve.
- The timestamp must differ from server time by at most 60 seconds, in either direction.
  The server checks it before and after reading the body. It also checks the body hash,
  method, policy expiry, and revocation before it runs the RPC.
- The proof prevents use for a different request. An identical request can still be replayed
  while the token is valid; there is no single-use check. Active streams continue after
  their initial request passes authentication.
