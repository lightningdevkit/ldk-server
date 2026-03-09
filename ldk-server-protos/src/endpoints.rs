// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

pub const GET_NODE_INFO_PATH: &str = "GetNodeInfo";
pub const GET_BALANCES_PATH: &str = "GetBalances";
pub const ONCHAIN_RECEIVE_PATH: &str = "OnchainReceive";
pub const ONCHAIN_SEND_PATH: &str = "OnchainSend";
pub const BOLT11_RECEIVE_PATH: &str = "Bolt11Receive";
pub const BOLT11_SEND_PATH: &str = "Bolt11Send";
pub const BOLT12_RECEIVE_PATH: &str = "Bolt12Receive";
pub const BOLT12_SEND_PATH: &str = "Bolt12Send";
pub const OPEN_CHANNEL_PATH: &str = "OpenChannel";
pub const SPLICE_IN_PATH: &str = "SpliceIn";
pub const SPLICE_OUT_PATH: &str = "SpliceOut";
pub const CLOSE_CHANNEL_PATH: &str = "CloseChannel";
pub const FORCE_CLOSE_CHANNEL_PATH: &str = "ForceCloseChannel";
pub const LIST_CHANNELS_PATH: &str = "ListChannels";
pub const LIST_PAYMENTS_PATH: &str = "ListPayments";
pub const LIST_FORWARDED_PAYMENTS_PATH: &str = "ListForwardedPayments";
pub const UPDATE_CHANNEL_CONFIG_PATH: &str = "UpdateChannelConfig";
pub const GET_PAYMENT_DETAILS_PATH: &str = "GetPaymentDetails";
pub const CONNECT_PEER_PATH: &str = "ConnectPeer";
pub const DISCONNECT_PEER_PATH: &str = "DisconnectPeer";
pub const SPONTANEOUS_SEND_PATH: &str = "SpontaneousSend";
pub const SIGN_MESSAGE_PATH: &str = "SignMessage";
pub const VERIFY_SIGNATURE_PATH: &str = "VerifySignature";
pub const EXPORT_PATHFINDING_SCORES_PATH: &str = "ExportPathfindingScores";
pub const GRAPH_LIST_CHANNELS_PATH: &str = "GraphListChannels";
pub const GRAPH_GET_CHANNEL_PATH: &str = "GraphGetChannel";
pub const GRAPH_LIST_NODES_PATH: &str = "GraphListNodes";
pub const GRAPH_GET_NODE_PATH: &str = "GraphGetNode";
pub const CREATE_API_KEY_PATH: &str = "CreateApiKey";
pub const GET_PERMISSIONS_PATH: &str = "GetPermissions";

/// All valid endpoint names. Used to validate API key permissions.
pub const ALL_ENDPOINTS: [&str; 30] = [
	GET_NODE_INFO_PATH,
	GET_BALANCES_PATH,
	ONCHAIN_RECEIVE_PATH,
	ONCHAIN_SEND_PATH,
	BOLT11_RECEIVE_PATH,
	BOLT11_SEND_PATH,
	BOLT12_RECEIVE_PATH,
	BOLT12_SEND_PATH,
	OPEN_CHANNEL_PATH,
	SPLICE_IN_PATH,
	SPLICE_OUT_PATH,
	CLOSE_CHANNEL_PATH,
	FORCE_CLOSE_CHANNEL_PATH,
	LIST_CHANNELS_PATH,
	LIST_PAYMENTS_PATH,
	LIST_FORWARDED_PAYMENTS_PATH,
	UPDATE_CHANNEL_CONFIG_PATH,
	GET_PAYMENT_DETAILS_PATH,
	CONNECT_PEER_PATH,
	DISCONNECT_PEER_PATH,
	SPONTANEOUS_SEND_PATH,
	SIGN_MESSAGE_PATH,
	VERIFY_SIGNATURE_PATH,
	EXPORT_PATHFINDING_SCORES_PATH,
	GRAPH_LIST_CHANNELS_PATH,
	GRAPH_GET_CHANNEL_PATH,
	GRAPH_LIST_NODES_PATH,
	GRAPH_GET_NODE_PATH,
	CREATE_API_KEY_PATH,
	GET_PERMISSIONS_PATH,
];
