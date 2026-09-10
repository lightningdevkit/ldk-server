// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

pub const ADMIN_PERMISSION: &str = "admin";
pub const NODE_READ_PERMISSION: &str = "node:read";
pub const ONCHAIN_RECEIVE_PERMISSION: &str = "onchain:receive";
pub const ONCHAIN_SEND_PERMISSION: &str = "onchain:send";
pub const INVOICES_CREATE_PERMISSION: &str = "invoices:create";
pub const PAYMENTS_READ_PERMISSION: &str = "payments:read";
pub const PAYMENTS_CLAIM_PERMISSION: &str = "payments:claim";
pub const PAYMENTS_SEND_PERMISSION: &str = "payments:send";
pub const CHANNELS_READ_PERMISSION: &str = "channels:read";
pub const CHANNELS_SPLICE_PERMISSION: &str = "channels:splice";
pub const CHANNELS_MANAGE_PERMISSION: &str = "channels:manage";
pub const CHANNELS_FORCE_CLOSE_PERMISSION: &str = "channels:force_close";
pub const PEERS_READ_PERMISSION: &str = "peers:read";
pub const PEERS_MANAGE_PERMISSION: &str = "peers:manage";
pub const MESSAGES_SIGN_PERMISSION: &str = "messages:sign";
pub const MESSAGES_VERIFY_PERMISSION: &str = "messages:verify";
pub const GRAPH_READ_PERMISSION: &str = "graph:read";
pub const UTILITIES_READ_PERMISSION: &str = "utilities:read";
pub const EVENTS_READ_PERMISSION: &str = "events:read";
pub const API_KEYS_MANAGE_PERMISSION: &str = "api_keys:manage";

/// All permissions accepted when an API key is created.
pub const ALL_PERMISSIONS: [&str; 20] = [
	ADMIN_PERMISSION,
	NODE_READ_PERMISSION,
	ONCHAIN_RECEIVE_PERMISSION,
	ONCHAIN_SEND_PERMISSION,
	INVOICES_CREATE_PERMISSION,
	PAYMENTS_READ_PERMISSION,
	PAYMENTS_CLAIM_PERMISSION,
	PAYMENTS_SEND_PERMISSION,
	CHANNELS_READ_PERMISSION,
	CHANNELS_MANAGE_PERMISSION,
	CHANNELS_SPLICE_PERMISSION,
	CHANNELS_FORCE_CLOSE_PERMISSION,
	PEERS_READ_PERMISSION,
	PEERS_MANAGE_PERMISSION,
	MESSAGES_SIGN_PERMISSION,
	MESSAGES_VERIFY_PERMISSION,
	GRAPH_READ_PERMISSION,
	UTILITIES_READ_PERMISSION,
	EVENTS_READ_PERMISSION,
	API_KEYS_MANAGE_PERMISSION,
];

/// Permissions included in the CLI `readonly` preset.
pub const READONLY_PERMISSIONS: [&str; 8] = [
	NODE_READ_PERMISSION,
	PAYMENTS_READ_PERMISSION,
	CHANNELS_READ_PERMISSION,
	PEERS_READ_PERMISSION,
	MESSAGES_VERIFY_PERMISSION,
	GRAPH_READ_PERMISSION,
	UTILITIES_READ_PERMISSION,
	EVENTS_READ_PERMISSION,
];

/// Permissions included in the CLI `invoice` preset.
pub const INVOICE_PERMISSIONS: [&str; 11] = [
	NODE_READ_PERMISSION,
	ONCHAIN_RECEIVE_PERMISSION,
	INVOICES_CREATE_PERMISSION,
	PAYMENTS_READ_PERMISSION,
	PAYMENTS_CLAIM_PERMISSION,
	CHANNELS_READ_PERMISSION,
	PEERS_READ_PERMISSION,
	MESSAGES_VERIFY_PERMISSION,
	GRAPH_READ_PERMISSION,
	UTILITIES_READ_PERMISSION,
	EVENTS_READ_PERMISSION,
];
