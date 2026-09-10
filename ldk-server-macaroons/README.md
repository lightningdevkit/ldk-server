# ldk-server-macaroons

Shared macaroon code for LDK Server and its clients. Supports v2 tokens, first-party caveats,
and request binding. Signatures use HMAC-SHA256 with constant-time verification.

- `Macaroon::from_hex` parses a token. `verify_signature` checks its signature.
- `parse_reusable_macaroon` rejects request tokens and checks room for request binding.
- `derive_macaroon` makes a restricted copy without contacting the server.
- `bind_macaroon_to_request` makes a token tied to a method, body, and the current time.

The server must still check every caveat, permissions, expiry, and revocation.
It also owns root generation and storage. This crate has no storage or networking code.

See the [API guide](https://github.com/lightningdevkit/ldk-server/blob/main/docs/api-guide.md#authentication)
for usage and supported restrictions, and the
[request proof format](https://github.com/lightningdevkit/ldk-server/blob/main/docs/request-binding.md)
for exact encoding rules.
