# ldk-server-client

Async Rust client library for communicating with an [LDK Server](https://github.com/lightningdevkit/ldk-server)
node over gRPC. Uses `reqwest` for unary RPCs and `hyper` for server-streaming (event
subscriptions).

## Usage

```rust,no_run
use ldk_server_client::client::LdkServerClient;
use ldk_server_client::ldk_server_grpc::api::GetNodeInfoRequest;

# #[tokio::main]
# async fn main() {
let cert_pem = std::fs::read("/path/to/tls.crt").unwrap();
let api_key = "your_hex_api_key".to_string();

let client = LdkServerClient::new(
    "localhost:3536".to_string(),
    api_key,
    &cert_pem,
).unwrap();

let info = client.get_node_info(GetNodeInfoRequest {}).await.unwrap();
println!("Node ID: {}", info.node_id);
# }
```

## Authentication

The client handles HMAC-SHA256 authentication automatically. Pass the hex-encoded API key
(found at `<storage_dir>/<network>/api_key`) and the server's TLS certificate (found at
`<storage_dir>/tls.crt`).

## Event Streaming

Subscribe to real-time payment events:

```rust,no_run
# use ldk_server_client::client::LdkServerClient;
# #[tokio::main]
# async fn main() {
# let cert_pem = std::fs::read("/path/to/tls.crt").unwrap();
# let client = LdkServerClient::new("localhost:3536".to_string(), "key".to_string(), &cert_pem).unwrap();
let mut stream = client.subscribe_events().await.unwrap();
while let Some(result) = stream.next_message().await {
    match result {
        Ok(event) => println!("Event: {:?}", event),
        Err(e) => eprintln!("Error: {}", e),
    }
}
# }
```

## Features

- **`serde`**: Enables `serde::Serialize` and `serde::Deserialize` on all proto types
  (via `ldk-server-grpc/serde`). Useful for JSON serialization.

## Error Handling

All methods return `Result<T, LdkServerError>`. Error codes map to gRPC status codes:

| `LdkServerErrorCode`  | gRPC Code               | Meaning                   |
|-----------------------|-------------------------|---------------------------|
| `InvalidRequestError` | INVALID_ARGUMENT (3)    | Bad request parameters    |
| `LightningError`      | FAILED_PRECONDITION (9) | Lightning operation error |
| `InternalServerError` | INTERNAL (13)           | Server bug                |
| `AuthError`           | UNAUTHENTICATED (16)    | Invalid credentials       |

## Documentation

- [API Guide](../docs/api-guide.md): full endpoint reference, auth details, and usage patterns
