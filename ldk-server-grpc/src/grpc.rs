// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! gRPC wire protocol primitives implemented directly on HTTP/2,
//! without depending on tonic or any gRPC framework.
//!
//! Reference: <https://github.com/grpc/grpc/blob/master/doc/PROTOCOL-HTTP2.md>

use bytes::{BufMut, Bytes, BytesMut};

// gRPC status codes (a subset — only those we use).
pub const GRPC_STATUS_INVALID_ARGUMENT: u32 = 3;
pub const GRPC_STATUS_DEADLINE_EXCEEDED: u32 = 4;
pub const GRPC_STATUS_FAILED_PRECONDITION: u32 = 9;
pub const GRPC_STATUS_UNIMPLEMENTED: u32 = 12;
pub const GRPC_STATUS_INTERNAL: u32 = 13;
pub const GRPC_STATUS_UNAVAILABLE: u32 = 14;
pub const GRPC_STATUS_UNAUTHENTICATED: u32 = 16;

/// A gRPC status with code and human-readable message.
#[derive(Debug)]
pub struct GrpcStatus {
	pub code: u32,
	pub message: String,
}

impl GrpcStatus {
	pub fn new(code: u32, message: impl Into<String>) -> Self {
		Self { code, message: message.into() }
	}
}

/// Decode a gRPC-framed request body, returning the inner protobuf bytes.
///
/// gRPC framing: 1 byte compressed flag + 4 bytes big-endian length + payload.
pub fn decode_grpc_body(bytes: &[u8]) -> Result<&[u8], GrpcStatus> {
	if bytes.len() < 5 {
		return Err(GrpcStatus::new(
			GRPC_STATUS_INVALID_ARGUMENT,
			"Request body too short for gRPC frame",
		));
	}

	// gRPC Compressed-Flag: 0 = uncompressed, 1 = compressed per grpc-encoding header.
	// We don't support compression because our RPCs exchange small protobuf messages where
	// compression overhead would outweigh savings. Returning UNIMPLEMENTED causes compliant
	// clients to retry without compression.
	let compressed = bytes[0];
	if compressed != 0 {
		return Err(GrpcStatus::new(
			GRPC_STATUS_UNIMPLEMENTED,
			"gRPC compression is not supported",
		));
	}

	let len = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
	if bytes.len() < 5 + len {
		return Err(GrpcStatus::new(
			GRPC_STATUS_INVALID_ARGUMENT,
			"gRPC frame length exceeds body size",
		));
	}

	if bytes.len() > 5 + len {
		return Err(GrpcStatus::new(
			GRPC_STATUS_INVALID_ARGUMENT,
			"Trailing data after gRPC frame",
		));
	}

	Ok(&bytes[5..5 + len])
}

/// Encode a protobuf message into a gRPC-framed `Bytes`.
///
/// gRPC framing: 1 byte compressed flag (0) + 4 bytes big-endian length + payload.
pub fn encode_grpc_frame(proto_bytes: &[u8]) -> Bytes {
	debug_assert!(
		proto_bytes.len() <= u32::MAX as usize,
		"gRPC message exceeds maximum frame size (4 GB)"
	);
	let mut buf = BytesMut::with_capacity(5 + proto_bytes.len());
	buf.put_u8(0); // no compression
	buf.put_u32(proto_bytes.len() as u32);
	buf.put_slice(proto_bytes);
	buf.freeze()
}

/// A response body type for gRPC over HTTP/2.
///
/// Implements `http_body::Body` to deliver gRPC-framed data followed by trailers.
pub enum GrpcBody {
	/// A single gRPC-framed message followed by OK trailers.
	Unary { data: Option<Bytes>, trailers_sent: bool },
	/// Empty body for Trailers-Only responses (error status is in the HTTP response headers).
	Empty,
	/// Multiple gRPC-framed messages streamed from a channel, followed by trailers.
	/// Send `Err(GrpcStatus)` to terminate the stream with an error status.
	Stream { rx: tokio::sync::mpsc::Receiver<Result<Bytes, GrpcStatus>>, done: bool },
	/// Plain (non-gRPC) response body with no trailers, used for non-RPC endpoints like metrics.
	Plain { data: Option<Bytes> },
}

impl http_body::Body for GrpcBody {
	type Data = Bytes;
	type Error = std::convert::Infallible;

	fn poll_frame(
		self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
		use std::task::Poll;

		let this = self.get_mut();
		match this {
			GrpcBody::Unary { data, trailers_sent } => {
				if let Some(bytes) = data.take() {
					Poll::Ready(Some(Ok(http_body::Frame::data(bytes))))
				} else if !*trailers_sent {
					*trailers_sent = true;
					Poll::Ready(Some(Ok(http_body::Frame::trailers(ok_trailers()))))
				} else {
					Poll::Ready(None)
				}
			},
			GrpcBody::Empty => Poll::Ready(None),
			GrpcBody::Stream { rx, done } => {
				if *done {
					return Poll::Ready(None);
				}
				match rx.poll_recv(_cx) {
					Poll::Ready(Some(Ok(bytes))) => {
						Poll::Ready(Some(Ok(http_body::Frame::data(bytes))))
					},
					Poll::Ready(Some(Err(status))) => {
						*done = true;
						Poll::Ready(Some(Ok(http_body::Frame::trailers(error_trailers(&status)))))
					},
					Poll::Ready(None) => {
						// Channel closed normally — send OK trailers
						*done = true;
						Poll::Ready(Some(Ok(http_body::Frame::trailers(ok_trailers()))))
					},
					Poll::Pending => Poll::Pending,
				}
			},
			GrpcBody::Plain { data } => match data.take() {
				Some(bytes) => Poll::Ready(Some(Ok(http_body::Frame::data(bytes)))),
				None => Poll::Ready(None),
			},
		}
	}
}

/// Build trailers for a successful gRPC response.
fn ok_trailers() -> http::HeaderMap {
	let mut trailers = http::HeaderMap::with_capacity(1);
	trailers.insert("grpc-status", http::HeaderValue::from_static("0"));
	trailers
}

/// Build trailers for a gRPC error response.
fn error_trailers(status: &GrpcStatus) -> http::HeaderMap {
	let mut trailers = http::HeaderMap::with_capacity(2);
	trailers.insert("grpc-status", http::HeaderValue::from_str(&status.code.to_string()).unwrap());
	if !status.message.is_empty() {
		// Percent-encode the message per gRPC spec.
		let encoded = percent_encode(&status.message);
		if let Ok(val) = http::HeaderValue::from_str(&encoded) {
			trailers.insert("grpc-message", val);
		}
	}
	trailers
}

/// Build a Trailers-Only gRPC error response.
///
/// Per the gRPC spec, error responses with no body encode `grpc-status` and `grpc-message`
/// in the HTTP response headers so the entire response is a single HEADERS frame with
/// END_STREAM. This is required for compatibility with strict client implementations
/// (grpc-go, grpc-java).
pub fn grpc_error_response(status: GrpcStatus) -> http::Response<GrpcBody> {
	let mut builder = http::Response::builder()
		.status(200)
		.header("content-type", "application/grpc+proto")
		.header("grpc-accept-encoding", "identity")
		.header("grpc-status", status.code.to_string());
	if !status.message.is_empty() {
		let encoded = percent_encode(&status.message);
		if let Ok(val) = http::HeaderValue::from_str(&encoded) {
			builder = builder.header("grpc-message", val);
		}
	}
	builder.body(GrpcBody::Empty).unwrap()
}

/// Build an HTTP 200 response with gRPC content-type and the given body.
pub fn grpc_response(body: GrpcBody) -> http::Response<GrpcBody> {
	http::Response::builder()
		.status(200)
		.header("content-type", "application/grpc+proto")
		.header("grpc-accept-encoding", "identity")
		.body(body)
		.unwrap()
}

/// Validate that the request looks like a gRPC call.
pub fn validate_grpc_request<B>(req: &http::Request<B>) -> Result<(), GrpcStatus> {
	if req.method() != http::Method::POST {
		return Err(GrpcStatus::new(GRPC_STATUS_UNIMPLEMENTED, "gRPC requires POST method"));
	}

	let content_type =
		req.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("");

	if content_type != "application/grpc" && content_type != "application/grpc+proto" {
		return Err(GrpcStatus::new(GRPC_STATUS_INVALID_ARGUMENT, "Invalid content-type for gRPC"));
	}

	Ok(())
}

/// Minimal percent-encoding for grpc-message (RFC 3986 unreserved chars pass through).
pub fn percent_encode(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	for b in s.bytes() {
		match b {
			b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b' ' => {
				out.push(b as char)
			},
			_ => {
				out.push('%');
				out.push(char::from(b"0123456789ABCDEF"[(b >> 4) as usize]));
				out.push(char::from(b"0123456789ABCDEF"[(b & 0xf) as usize]));
			},
		}
	}
	out
}

/// Minimal percent-decoding for grpc-message values.
pub fn percent_decode(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	let mut chars = s.bytes();
	while let Some(b) = chars.next() {
		if b == b'%' {
			let hi = chars.next().and_then(hex_val);
			let lo = chars.next().and_then(hex_val);
			if let (Some(h), Some(l)) = (hi, lo) {
				out.push(((h << 4) | l) as char);
			}
		} else {
			out.push(b as char);
		}
	}
	out
}

fn hex_val(b: u8) -> Option<u8> {
	match b {
		b'0'..=b'9' => Some(b - b'0'),
		b'A'..=b'F' => Some(b - b'A' + 10),
		b'a'..=b'f' => Some(b - b'a' + 10),
		_ => None,
	}
}

/// Parse the `grpc-timeout` header value into a `Duration`.
///
/// Format: `<number><unit>` where unit is one of:
/// `H` (hours), `M` (minutes), `S` (seconds), `m` (milliseconds),
/// `u` (microseconds), `n` (nanoseconds).
pub fn parse_grpc_timeout(value: &str) -> Result<std::time::Duration, GrpcStatus> {
	if !(2..=9).contains(&value.len()) {
		return Err(GrpcStatus::new(GRPC_STATUS_INVALID_ARGUMENT, "Invalid grpc-timeout header"));
	}

	let (num_str, unit) = value.split_at(value.len() - 1);
	let num: u64 = num_str.parse().map_err(|_| {
		GrpcStatus::new(GRPC_STATUS_INVALID_ARGUMENT, "Invalid grpc-timeout header")
	})?;

	let duration = match unit {
		"H" => num.checked_mul(3600).map(std::time::Duration::from_secs),
		"M" => num.checked_mul(60).map(std::time::Duration::from_secs),
		"S" => Some(std::time::Duration::from_secs(num)),
		"m" => Some(std::time::Duration::from_millis(num)),
		"u" => Some(std::time::Duration::from_micros(num)),
		"n" => Some(std::time::Duration::from_nanos(num)),
		_ => None,
	};

	duration
		.ok_or_else(|| GrpcStatus::new(GRPC_STATUS_INVALID_ARGUMENT, "Invalid grpc-timeout header"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_encode_decode_roundtrip() {
		let payload = b"hello world";
		let encoded = encode_grpc_frame(payload);
		let decoded = decode_grpc_body(&encoded).unwrap();
		assert_eq!(decoded, payload);
	}

	#[test]
	fn test_encode_empty() {
		let encoded = encode_grpc_frame(b"");
		assert_eq!(encoded.len(), 5);
		assert_eq!(&encoded[..5], &[0, 0, 0, 0, 0]);
		let decoded = decode_grpc_body(&encoded).unwrap();
		assert!(decoded.is_empty());
	}

	#[test]
	fn test_decode_too_short() {
		assert!(decode_grpc_body(&[0, 0, 0]).is_err());
	}

	#[test]
	fn test_decode_compressed_rejected() {
		let data = vec![1u8, 0, 0, 0, 1, 42]; // compressed flag = 1
		let result = decode_grpc_body(&data);
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().code, GRPC_STATUS_UNIMPLEMENTED);
	}

	#[test]
	fn test_decode_length_exceeds_body() {
		let data = vec![0u8, 0, 0, 0, 10, 1, 2]; // claims 10 bytes, only 2 present
		let result = decode_grpc_body(&data);
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().code, GRPC_STATUS_INVALID_ARGUMENT);
	}

	#[test]
	fn test_decode_trailing_data_rejected() {
		let data = vec![0u8, 0, 0, 0, 1, 42, 99]; // 1-byte payload + trailing byte
		let result = decode_grpc_body(&data);
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().code, GRPC_STATUS_INVALID_ARGUMENT);
	}

	#[test]
	fn test_decode_rejects_all_short_inputs() {
		for len in 0..5 {
			let data = vec![0u8; len];
			assert!(decode_grpc_body(&data).is_err(), "should reject {len}-byte input");
		}
	}

	#[test]
	fn test_percent_encode() {
		assert_eq!(percent_encode("hello"), "hello");
		assert_eq!(percent_encode("a/b"), "a%2Fb");
		assert_eq!(percent_encode("100%"), "100%25");
	}

	#[test]
	fn test_percent_encode_ascii_range() {
		// Verify every ASCII byte is either passed through or percent-encoded
		for b in 0u8..=127 {
			let s = String::from(b as char);
			let encoded = percent_encode(&s);
			let is_unreserved = b.is_ascii_alphanumeric()
				|| b == b'-' || b == b'_'
				|| b == b'.' || b == b'~'
				|| b == b' ';
			if is_unreserved {
				assert_eq!(encoded, s, "byte {b:#04x} ({}) should pass through", b as char);
			} else {
				assert_eq!(encoded, format!("%{b:02X}"), "byte {b:#04x} should be percent-encoded");
			}
		}
	}

	#[test]
	fn test_percent_encode_multibyte_utf8() {
		// Each byte of multi-byte UTF-8 chars should be individually encoded
		let encoded = percent_encode("café");
		assert_eq!(encoded, "caf%C3%A9");
	}

	#[test]
	fn test_percent_decode_plain_text() {
		assert_eq!(percent_decode("hello"), "hello");
		assert_eq!(percent_decode(""), "");
	}

	#[test]
	fn test_percent_decode_encoded_chars() {
		assert_eq!(percent_decode("hello%20world"), "hello world");
		assert_eq!(percent_decode("100%25"), "100%");
		assert_eq!(percent_decode("%2F"), "/");
		assert_eq!(percent_decode("%2f"), "/");
	}

	#[test]
	fn test_percent_decode_roundtrip_with_server_encode() {
		assert_eq!(percent_decode("a%2Fb"), "a/b");
		assert_eq!(percent_decode("caf%C3%A9"), "caf\u{00c3}\u{00a9}");
		assert_eq!(percent_decode("hello%20world%21"), "hello world!");
	}

	#[test]
	fn test_percent_decode_truncated_sequence() {
		assert_eq!(percent_decode("abc%2"), "abc");
		assert_eq!(percent_decode("abc%"), "abc");
	}

	#[test]
	fn test_percent_decode_invalid_hex() {
		assert_eq!(percent_decode("%GG"), "");
		assert_eq!(percent_decode("%ZZ"), "");
	}

	#[test]
	fn test_percent_decode_all_ascii_values() {
		for byte in 0u8..=127 {
			let encoded = format!("%{byte:02X}");
			let decoded = percent_decode(&encoded);
			assert_eq!(decoded.as_bytes(), &[byte], "failed for %{byte:02X}");
		}
	}

	#[test]
	fn test_hex_val() {
		for b in b'0'..=b'9' {
			assert_eq!(hex_val(b), Some(b - b'0'));
		}
		for (i, b) in (b'A'..=b'F').enumerate() {
			assert_eq!(hex_val(b), Some(10 + i as u8));
		}
		for (i, b) in (b'a'..=b'f').enumerate() {
			assert_eq!(hex_val(b), Some(10 + i as u8));
		}
		assert_eq!(hex_val(b'G'), None);
		assert_eq!(hex_val(b'g'), None);
		assert_eq!(hex_val(b' '), None);
		assert_eq!(hex_val(b'/'), None);
		assert_eq!(hex_val(b':'), None);
		assert_eq!(hex_val(b'@'), None);
	}

	#[test]
	fn test_parse_grpc_timeout() {
		use std::time::Duration;
		assert_eq!(parse_grpc_timeout("5S").unwrap(), Duration::from_secs(5));
		assert_eq!(parse_grpc_timeout("500m").unwrap(), Duration::from_millis(500));
		assert_eq!(parse_grpc_timeout("1H").unwrap(), Duration::from_secs(3600));
		assert_eq!(parse_grpc_timeout("30M").unwrap(), Duration::from_secs(1800));
		assert_eq!(parse_grpc_timeout("100u").unwrap(), Duration::from_micros(100));
		assert_eq!(parse_grpc_timeout("1000n").unwrap(), Duration::from_nanos(1000));
		assert!(parse_grpc_timeout("").is_err());
		assert!(parse_grpc_timeout("S").is_err());
		assert!(parse_grpc_timeout("5x").is_err());
	}

	#[test]
	fn test_parse_grpc_timeout_boundary_values() {
		use std::time::Duration;
		// Zero values
		assert_eq!(parse_grpc_timeout("0S").unwrap(), Duration::from_secs(0));
		assert_eq!(parse_grpc_timeout("0m").unwrap(), Duration::from_millis(0));
		// Large values
		assert_eq!(parse_grpc_timeout("99999999S").unwrap(), Duration::from_secs(99_999_999));
		// Various invalid formats
		assert!(parse_grpc_timeout("5").is_err()); // no unit
		assert!(parse_grpc_timeout("abc").is_err()); // non-numeric
		assert!(parse_grpc_timeout("5X").is_err()); // unknown unit
		assert!(parse_grpc_timeout("5 S").is_err()); // space before unit
	}

	#[test]
	fn test_parse_grpc_timeout_rejects_too_many_digits() {
		let err = parse_grpc_timeout("100000000S").unwrap_err();
		assert_eq!(err.code, GRPC_STATUS_INVALID_ARGUMENT);

		let err = parse_grpc_timeout("18446744073709551615H").unwrap_err();
		assert_eq!(err.code, GRPC_STATUS_INVALID_ARGUMENT);
	}

	mod tonic_compat {
		use std::convert::Infallible;
		use std::future::Future;
		use std::net::SocketAddr;
		use std::pin::Pin;

		use http_body_util::BodyExt;
		use hyper::body::Incoming;
		use hyper::server::conn::http2;
		use hyper::service::Service;
		use hyper::{Request, Response};
		use hyper_util::rt::TokioExecutor;
		use prost::Message;
		use tokio::net::TcpListener;
		use tokio::sync::mpsc as tokio_mpsc;

		use super::*;
		use crate::api::{GetNodeInfoRequest, GetNodeInfoResponse};
		use crate::endpoints::{GET_NODE_INFO_PATH, GRPC_SERVICE_PREFIX, SUBSCRIBE_EVENTS_PATH};
		use crate::events::EventEnvelope;

		const TEST_NODE_ID: &str =
			"02deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefde";

		/// Build a full gRPC path from a method name constant.
		fn grpc_path(method: &str) -> String {
			format!("{GRPC_SERVICE_PREFIX}{method}")
		}

		/// Minimal gRPC service for testing. Uses the real GrpcBody, grpc_response,
		/// grpc_error_response, and validate_grpc_request from the server code.
		#[derive(Clone)]
		struct TestGrpcService;

		impl Service<Request<Incoming>> for TestGrpcService {
			type Response = Response<GrpcBody>;
			type Error = Infallible;
			type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

			fn call(&self, req: Request<Incoming>) -> Self::Future {
				Box::pin(async move {
					// Use the real gRPC validation
					if let Err(status) = validate_grpc_request(&req) {
						return Ok(grpc_error_response(status));
					}

					let path = req.uri().path().to_string();
					let method = match path.strip_prefix(GRPC_SERVICE_PREFIX) {
						Some(m) => m.to_string(),
						None => {
							return Ok(grpc_error_response(GrpcStatus::new(
								GRPC_STATUS_UNIMPLEMENTED,
								format!("Unknown path: {path}"),
							)));
						},
					};

					match method.as_str() {
						GET_NODE_INFO_PATH => {
							// Read body and decode gRPC frame
							let body_bytes = match req.into_body().collect().await {
								Ok(collected) => collected.to_bytes(),
								Err(_) => {
									return Ok(grpc_error_response(GrpcStatus::new(
										GRPC_STATUS_INTERNAL,
										"Failed to read body",
									)));
								},
							};

							let proto_bytes = match decode_grpc_body(&body_bytes) {
								Ok(b) => b,
								Err(status) => return Ok(grpc_error_response(status)),
							};

							if GetNodeInfoRequest::decode(proto_bytes).is_err() {
								return Ok(grpc_error_response(GrpcStatus::new(
									GRPC_STATUS_INVALID_ARGUMENT,
									"Malformed request",
								)));
							}

							// Return a hardcoded response
							let response = GetNodeInfoResponse {
								node_id: TEST_NODE_ID.to_string(),
								..Default::default()
							};
							let encoded = encode_grpc_frame(&response.encode_to_vec());
							Ok(grpc_response(GrpcBody::Unary {
								data: Some(encoded),
								trailers_sent: false,
							}))
						},
						SUBSCRIBE_EVENTS_PATH => {
							let (tx, rx) = tokio_mpsc::channel(16);

							// Spawn a task that sends a few events then closes
							tokio::spawn(async move {
								for _ in 0..3 {
									let event = EventEnvelope::default();
									let frame = encode_grpc_frame(&event.encode_to_vec());
									if tx.send(Ok(frame)).await.is_err() {
										return;
									}
								}
								// Channel closes => OK trailers
							});

							Ok(grpc_response(GrpcBody::Stream { rx, done: false }))
						},
						"SubscribeEventsError" => {
							let (tx, rx) = tokio_mpsc::channel(16);

							tokio::spawn(async move {
								// Send one event then an error
								let event = EventEnvelope::default();
								let frame = encode_grpc_frame(&event.encode_to_vec());
								let _ = tx.send(Ok(frame)).await;
								let _ = tx
									.send(Err(GrpcStatus::new(
										GRPC_STATUS_UNAVAILABLE,
										"Server shutting down",
									)))
									.await;
							});

							Ok(grpc_response(GrpcBody::Stream { rx, done: false }))
						},
						"ErrorWithSpecialChars" => Ok(grpc_error_response(GrpcStatus::new(
							GRPC_STATUS_INVALID_ARGUMENT,
							"bad request: field/value has special chars (100%)",
						))),
						_ => Ok(grpc_error_response(GrpcStatus::new(
							GRPC_STATUS_UNIMPLEMENTED,
							format!("Unknown method: {method}"),
						))),
					}
				})
			}
		}

		/// Start a plaintext h2c test server on a random port.
		async fn start_test_server() -> SocketAddr {
			let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
			let addr = listener.local_addr().unwrap();

			tokio::spawn(async move {
				loop {
					let (stream, _) = match listener.accept().await {
						Ok(conn) => conn,
						Err(_) => continue,
					};
					let io = hyper_util::rt::TokioIo::new(stream);
					tokio::spawn(async move {
						let _ = http2::Builder::new(TokioExecutor::new())
							.serve_connection(io, TestGrpcService)
							.await;
					});
				}
			});

			addr
		}

		/// Connect a tonic Grpc client to the given address.
		async fn connect_client(
			addr: SocketAddr,
		) -> tonic::client::Grpc<tonic::transport::Channel> {
			let endpoint = tonic::transport::Endpoint::from_shared(format!("http://{addr}"))
				.unwrap()
				.connect_timeout(std::time::Duration::from_secs(5));
			let channel = endpoint.connect().await.unwrap();
			let mut client = tonic::client::Grpc::new(channel);
			client.ready().await.unwrap();
			client
		}

		#[tokio::test]
		async fn test_tonic_unary_success() {
			let addr = start_test_server().await;
			let mut client = connect_client(addr).await;

			let response: tonic::Response<GetNodeInfoResponse> = client
				.unary(
					tonic::Request::new(GetNodeInfoRequest {}),
					grpc_path(GET_NODE_INFO_PATH).parse().unwrap(),
					tonic::codec::ProstCodec::default(),
				)
				.await
				.unwrap();

			assert_eq!(response.get_ref().node_id, TEST_NODE_ID);
		}

		#[tokio::test]
		async fn test_tonic_unimplemented_method() {
			let addr = start_test_server().await;
			let mut client = connect_client(addr).await;

			let result: Result<tonic::Response<GetNodeInfoResponse>, tonic::Status> = client
				.unary(
					tonic::Request::new(GetNodeInfoRequest {}),
					grpc_path("NoSuchMethod").parse().unwrap(),
					tonic::codec::ProstCodec::default(),
				)
				.await;

			let status = result.unwrap_err();
			assert_eq!(status.code(), tonic::Code::Unimplemented);
			assert!(status.message().contains("Unknown method"));
		}

		#[tokio::test]
		async fn test_tonic_error_message_with_special_chars() {
			let addr = start_test_server().await;
			let mut client = connect_client(addr).await;

			let result: Result<tonic::Response<GetNodeInfoResponse>, tonic::Status> = client
				.unary(
					tonic::Request::new(GetNodeInfoRequest {}),
					grpc_path("ErrorWithSpecialChars").parse().unwrap(),
					tonic::codec::ProstCodec::default(),
				)
				.await;

			let status = result.unwrap_err();
			assert_eq!(status.code(), tonic::Code::InvalidArgument);
			assert_eq!(status.message(), "bad request: field/value has special chars (100%)");
		}

		#[tokio::test]
		async fn test_tonic_empty_request_response() {
			// GetNodeInfoRequest is an empty message — tests zero-length gRPC frame
			let addr = start_test_server().await;
			let mut client = connect_client(addr).await;

			let response: tonic::Response<GetNodeInfoResponse> = client
				.unary(
					tonic::Request::new(GetNodeInfoRequest {}),
					grpc_path(GET_NODE_INFO_PATH).parse().unwrap(),
					tonic::codec::ProstCodec::default(),
				)
				.await
				.unwrap();

			// Response is non-empty but request was empty — validates empty frame encoding
			assert!(!response.get_ref().node_id.is_empty());
		}

		#[tokio::test]
		async fn test_tonic_server_streaming_success() {
			use tokio_stream::StreamExt;

			let addr = start_test_server().await;
			let mut client = connect_client(addr).await;

			let response = client
				.server_streaming(
					tonic::Request::new(crate::api::SubscribeEventsRequest {}),
					grpc_path(SUBSCRIBE_EVENTS_PATH).parse().unwrap(),
					tonic::codec::ProstCodec::default(),
				)
				.await
				.unwrap();

			let mut stream = response.into_inner();
			let mut events: Vec<EventEnvelope> = Vec::new();
			while let Some(msg) = stream.next().await {
				events.push(msg.unwrap());
			}

			assert_eq!(events.len(), 3);
			for event in &events {
				assert_eq!(*event, EventEnvelope::default());
			}
		}

		#[tokio::test]
		async fn test_tonic_server_streaming_error_termination() {
			use tokio_stream::StreamExt;

			let addr = start_test_server().await;
			let mut client = connect_client(addr).await;

			let response = client
				.server_streaming(
					tonic::Request::new(crate::api::SubscribeEventsRequest {}),
					grpc_path("SubscribeEventsError").parse().unwrap(),
					tonic::codec::ProstCodec::default(),
				)
				.await
				.unwrap();

			let mut stream = response.into_inner();

			// First message should succeed
			let first: EventEnvelope = stream.next().await.unwrap().unwrap();
			assert_eq!(first, EventEnvelope::default());

			// Next should be the terminal error
			let err = stream.next().await.unwrap().unwrap_err();
			assert_eq!(err.code(), tonic::Code::Unavailable);
			assert_eq!(err.message(), "Server shutting down");
		}
	}
}
