// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

/// Write the authentication preimage to a hash engine or byte sink without allocating.
/// The key ID is fixed-width; the method is length-prefixed and the timestamp is big-endian.
pub fn write_auth_preimage(
	key_id: &str, method: &str, timestamp: u64, body: &[u8], mut write: impl FnMut(&[u8]),
) {
	write(b"ldk-server-auth-v1");
	write(key_id.as_bytes());
	write(&(method.len() as u64).to_be_bytes());
	write(method.as_bytes());
	write(&timestamp.to_be_bytes());
	write(body);
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn preimage_matches_wire_format() {
		let mut bytes = Vec::new();
		write_auth_preimage(
			"0123456789abcdef0123456789abcdef",
			"GetNodeInfo",
			7,
			b"body",
			|part| bytes.extend_from_slice(part),
		);
		assert_eq!(bytes, b"ldk-server-auth-v10123456789abcdef0123456789abcdef\x00\x00\x00\x00\x00\x00\x00\x0bGetNodeInfo\x00\x00\x00\x00\x00\x00\x00\x07body");
	}
}
