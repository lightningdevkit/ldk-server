// UniFFI-exposed types and client wrapper for `ldk-server-client`.
//
// The types in this module are hand-written flat analogues of the
// prost-generated protobuf types from `ldk-server-grpc`. They exist because
// prost types are not directly UniFFI-exportable: they use
// `#[derive(::prost::Message)]`, nested `oneof` modules, and `prost::bytes::Bytes`,
// none of which UniFFI can serialize across the FFI boundary.
//
// Conversions (`From`/`Into`) to and from the underlying prost types are
// implemented alongside each wrapper.
