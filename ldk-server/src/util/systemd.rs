// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

#[cfg(target_os = "linux")]
use std::os::linux::net::SocketAddrExt;
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixDatagram;

#[cfg(target_os = "linux")]
use log::{info, warn};

#[cfg(target_os = "linux")]
fn notify(state: &str) {
	let socket_path = match std::env::var("NOTIFY_SOCKET") {
		Ok(path) => path,
		Err(_) => return,
	};

	let socket = match UnixDatagram::unbound() {
		Ok(s) => s,
		Err(e) => {
			warn!("Failed to create socket for systemd notification: {e}");
			return;
		},
	};

	// systemd sets NOTIFY_SOCKET to either a filesystem path (e.g. /run/systemd/notify)
	// or an abstract socket prefixed with '@'. Abstract sockets require special addressing.
	let result = if let Some(abstract_name) = socket_path.strip_prefix('@') {
		match std::os::unix::net::SocketAddr::from_abstract_name(abstract_name) {
			Ok(addr) => socket.send_to_addr(state.as_bytes(), &addr),
			Err(e) => {
				warn!("Failed to create abstract socket address: {e}");
				return;
			},
		}
	} else {
		socket.send_to(state.as_bytes(), &socket_path)
	};

	if let Err(e) = result {
		warn!("Failed to send systemd notification: {e}");
	} else {
		info!("Sent systemd notification: {state}");
	}
}

#[cfg(target_os = "linux")]
pub fn notify_ready() {
	notify("READY=1");
}

#[cfg(target_os = "linux")]
pub fn notify_stopping() {
	notify("STOPPING=1");
}

#[cfg(not(target_os = "linux"))]
pub fn notify_ready() {}

#[cfg(not(target_os = "linux"))]
pub fn notify_stopping() {}
