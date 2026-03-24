// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::str::FromStr;
use std::time::Duration;

use e2e_tests::{
	find_available_port, mine_and_sync, run_cli, run_cli_raw, setup_funded_channel,
	wait_for_onchain_balance, LdkServerHandle, RabbitMqEventConsumer, TestBitcoind,
};
use hex_conservative::{DisplayHex, FromHex};
use ldk_node::bitcoin::hashes::{sha256, Hash};
use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::bitcoin::Network;
use ldk_node::lightning::ln::msgs::SocketAddress;
use ldk_node::lightning::offers::offer::Offer;
use ldk_node::lightning_invoice::Bolt11Invoice;
use ldk_server_json_models::api::{
	Bolt11ReceiveRequest, Bolt12ReceiveRequest, OnchainReceiveRequest,
};
use ldk_server_json_models::events::Event;
use ldk_server_json_models::types::Bolt11InvoiceDescription;

#[tokio::test]
async fn test_cli_get_node_info() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let output = run_cli(&server, &["get-node-info"]);
	assert_eq!(output["node_id"], server.node_id().to_lower_hex_string());

	// Verify block hash is a parseable 32-byte value and height is nonzero
	let block_hash_hex = output["current_best_block"]["block_hash"].as_str().unwrap();
	assert_eq!(block_hash_hex.len(), 64);
	<[u8; 32]>::from_hex(block_hash_hex).expect("block_hash should be valid 32-byte hex");
	assert!(output["current_best_block"]["height"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn test_cli_onchain_receive() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let output = run_cli(&server, &["onchain-receive"]);
	let address = output["address"].as_str().unwrap();
	assert!(address.starts_with("bcrt1"), "Expected regtest address, got: {}", address);
}

#[tokio::test]
async fn test_cli_get_balances() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let output = run_cli(&server, &["get-balances"]);
	assert_eq!(output["total_onchain_balance_sats"], 0);
	assert_eq!(output["spendable_onchain_balance_sats"], 0);
	assert_eq!(output["total_lightning_balance_sats"], 0);
}

#[tokio::test]
async fn test_cli_list_channels_empty() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let output = run_cli(&server, &["list-channels"]);
	assert!(output["channels"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_cli_list_payments_empty() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let output = run_cli(&server, &["list-payments"]);
	assert!(output["list"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_cli_list_forwarded_payments_empty() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let output = run_cli(&server, &["list-forwarded-payments"]);
	assert!(output["list"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_cli_sign_message() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let output = run_cli(&server, &["sign-message", "hello"]);
	assert!(!output["signature"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_cli_verify_signature() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let sign_output = run_cli(&server, &["sign-message", "hello"]);
	let signature = sign_output["signature"].as_str().unwrap();

	let node_id_hex = server.node_id().to_lower_hex_string();
	let output = run_cli(&server, &["verify-signature", "hello", signature, &node_id_hex]);
	assert_eq!(output["valid"], true);
}

#[tokio::test]
async fn test_cli_export_pathfinding_scores() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	// Make a payment so the scorer has data
	let invoice_resp = server_b
		.client()
		.bolt11_receive(Bolt11ReceiveRequest {
			amount_msat: Some(10_000_000),
			description: Some(Bolt11InvoiceDescription::Direct("test".to_string())),
			expiry_secs: 3600,
		})
		.await
		.unwrap();
	run_cli(&server_a, &["bolt11-send", &invoice_resp.invoice]);
	tokio::time::sleep(Duration::from_secs(3)).await;

	let output = run_cli(&server_a, &["export-pathfinding-scores"]);
	assert!(output.get("pathfinding_scores").is_some());
}

#[tokio::test]
async fn test_cli_bolt11_receive() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let output = run_cli(&server, &["bolt11-receive", "50000sat", "-d", "test"]);
	let invoice_str = output["invoice"].as_str().unwrap();
	assert!(invoice_str.starts_with("lnbcrt"), "Expected lnbcrt prefix, got: {}", invoice_str);

	let invoice: Bolt11Invoice = invoice_str.parse().unwrap();

	// Cross-check payment_hash bytes: API response vs parsed invoice
	let api_hash = <[u8; 32]>::from_hex(output["payment_hash"].as_str().unwrap()).unwrap();
	assert_eq!(
		api_hash,
		*invoice.payment_hash().as_byte_array(),
		"payment_hash bytes should match invoice"
	);

	// Cross-check payment_secret bytes: API response vs parsed invoice
	let api_secret = <[u8; 32]>::from_hex(output["payment_secret"].as_str().unwrap()).unwrap();
	assert_eq!(api_secret, invoice.payment_secret().0, "payment_secret bytes should match invoice");
}

#[tokio::test]
async fn test_cli_bolt12_receive() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	// BOLT12 offers need announced channels for blinded reply paths
	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	let output = run_cli(&server_a, &["bolt12-receive", "test offer"]);
	let offer_str = output["offer"].as_str().unwrap();
	assert!(offer_str.starts_with("lno"), "Expected lno prefix, got: {}", offer_str);

	let offer: Offer = offer_str.parse().unwrap();
	let offer_id = <[u8; 32]>::from_hex(output["offer_id"].as_str().unwrap()).unwrap();
	assert_eq!(offer.id().0, offer_id);
}

#[tokio::test]
async fn test_cli_onchain_send() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	// Fund the server
	let addr = server.client().onchain_receive(OnchainReceiveRequest {}).await.unwrap().address;
	bitcoind.fund_address(&addr, 1.0);
	mine_and_sync(&bitcoind, &[&server], 6).await;
	wait_for_onchain_balance(server.client(), Duration::from_secs(30)).await;

	// Get a destination address from the server itself
	let recv_output = run_cli(&server, &["onchain-receive"]);
	let dest_addr = recv_output["address"].as_str().unwrap();

	let output = run_cli(&server, &["onchain-send", dest_addr, "50000sat"]);
	let txid_hex = output["txid"].as_str().unwrap();
	assert_eq!(txid_hex.len(), 64);
	<[u8; 32]>::from_hex(txid_hex).expect("txid should be valid 32-byte hex");
}

#[tokio::test]
async fn test_cli_connect_peer() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;

	let addr = format!("127.0.0.1:{}", server_b.p2p_port);
	let node_id_hex = server_b.node_id().to_lower_hex_string();
	let output = run_cli(&server_a, &["connect-peer", &node_id_hex, &addr]);
	// ConnectPeerResponse is empty
	assert!(output.is_object());
}

#[tokio::test]
async fn test_cli_list_peers() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;

	let output = run_cli(&server_a, &["list-peers"]);
	assert!(output["peers"].as_array().unwrap().is_empty());
	let output = run_cli(&server_b, &["list-peers"]);
	assert!(output["peers"].as_array().unwrap().is_empty());

	let addr = format!("127.0.0.1:{}", server_b.p2p_port);
	let node_id_hex = server_b.node_id().to_lower_hex_string();
	run_cli(&server_a, &["connect-peer", &node_id_hex, &addr]);

	let output = run_cli(&server_a, &["list-peers"]);
	let peers = output["peers"].as_array().unwrap();
	assert_eq!(peers.len(), 1);
	assert_eq!(peers[0]["node_id"], node_id_hex);
	assert_eq!(peers[0]["address"], addr);
	assert_eq!(peers[0]["is_persisted"], false);
	assert_eq!(peers[0]["is_connected"], true);
}

// === CLI tests: Group 4 — Two-node with channel ===

#[tokio::test]
async fn test_cli_open_channel() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;

	// Fund both servers
	let addr_a = server_a.client().onchain_receive(OnchainReceiveRequest {}).await.unwrap().address;
	let addr_b = server_b.client().onchain_receive(OnchainReceiveRequest {}).await.unwrap().address;
	bitcoind.fund_address(&addr_a, 1.0);
	bitcoind.fund_address(&addr_b, 0.1);
	mine_and_sync(&bitcoind, &[&server_a, &server_b], 6).await;
	wait_for_onchain_balance(server_a.client(), Duration::from_secs(30)).await;
	wait_for_onchain_balance(server_b.client(), Duration::from_secs(30)).await;

	// Open channel via CLI
	let addr = format!("127.0.0.1:{}", server_b.p2p_port);
	let node_id_hex = server_b.node_id().to_lower_hex_string();
	let output = run_cli(
		&server_a,
		&["open-channel", &node_id_hex, &addr, "100000sat", "--announce-channel"],
	);
	assert!(!output["user_channel_id"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_cli_list_channels() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	let output = run_cli(&server_a, &["list-channels"]);
	let channels = output["channels"].as_array().unwrap();
	assert!(!channels.is_empty());

	let ch = &channels[0];
	// Verify counterparty_node_id is server_b's actual pubkey
	let cp_id = <[u8; 33]>::from_hex(ch["counterparty_node_id"].as_str().unwrap()).unwrap();
	assert_eq!(cp_id, *server_b.node_id(), "counterparty should be server_b");

	// Verify channel_id is a valid 32-byte value
	let channel_id = <[u8; 32]>::from_hex(ch["channel_id"].as_str().unwrap()).unwrap();
	assert_ne!(channel_id, [0u8; 32], "channel_id should not be all zeros");

	// Verify funding txo has a valid txid
	let funding_txid_hex = ch["funding_txo"]["txid"].as_str().unwrap();
	assert_eq!(funding_txid_hex.len(), 64);
	<[u8; 32]>::from_hex(funding_txid_hex).expect("funding txid should be valid 32-byte hex");

	// Both sides should see the same channel — cross-check from server_b
	let output_b = run_cli(&server_b, &["list-channels"]);
	let channels_b = output_b["channels"].as_array().unwrap();
	assert!(!channels_b.is_empty());
	let ch_b = &channels_b[0];
	let cp_id_b = <[u8; 33]>::from_hex(ch_b["counterparty_node_id"].as_str().unwrap()).unwrap();
	assert_eq!(cp_id_b, *server_a.node_id(), "server_b's counterparty should be server_a");
	assert_eq!(
		ch_b["funding_txo"]["txid"], ch["funding_txo"]["txid"],
		"both sides should report same funding txid"
	);
}

#[tokio::test]
async fn test_cli_update_channel_config() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	let user_channel_id = setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	let node_id_hex = server_b.node_id().to_lower_hex_string();
	let output = run_cli(
		&server_a,
		&[
			"update-channel-config",
			&user_channel_id,
			&node_id_hex,
			"--forwarding-fee-base-msat",
			"100",
		],
	);
	assert!(output.is_object());
}

#[tokio::test]
async fn test_cli_bolt11_send() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;

	// Set up event consumers before any payments
	let mut consumer_a = RabbitMqEventConsumer::new(&server_a.exchange_name).await;
	let mut consumer_b = RabbitMqEventConsumer::new(&server_b.exchange_name).await;

	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	// Create invoice on B via client lib
	let invoice_resp = server_b
		.client()
		.bolt11_receive(Bolt11ReceiveRequest {
			amount_msat: Some(10_000_000),
			description: Some(Bolt11InvoiceDescription::Direct("test".to_string())),
			expiry_secs: 3600,
		})
		.await
		.unwrap();

	// Pay via CLI from A
	let output = run_cli(&server_a, &["bolt11-send", &invoice_resp.invoice]);
	assert!(!output["payment_id"].as_str().unwrap().is_empty());

	// Verify events
	tokio::time::sleep(Duration::from_secs(5)).await;

	let events_a = consumer_a.consume_events(5, Duration::from_secs(10)).await;
	assert!(
		events_a.iter().any(|e| matches!(e, Event::PaymentSuccessful(_))),
		"Expected PaymentSuccessful on sender"
	);

	let events_b = consumer_b.consume_events(5, Duration::from_secs(10)).await;
	assert!(
		events_b.iter().any(|e| matches!(e, Event::PaymentReceived(_))),
		"Expected PaymentReceived on receiver"
	);
}

#[tokio::test]
async fn test_cli_pay() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	// Pay a BOLT11 invoice via unified `pay` command
	let invoice_resp = server_b
		.client()
		.bolt11_receive(Bolt11ReceiveRequest {
			amount_msat: Some(10_000_000),
			description: Some(Bolt11InvoiceDescription::Direct("test".to_string())),
			expiry_secs: 3600,
		})
		.await
		.unwrap();
	let output = run_cli(&server_a, &["pay", &invoice_resp.invoice]);
	assert!(output.get("bolt11_payment_id").is_some());

	// Pay a BOLT12 offer via unified `pay` command
	let offer_resp = server_b
		.client()
		.bolt12_receive(Bolt12ReceiveRequest {
			description: "test offer".to_string(),
			amount_msat: None,
			expiry_secs: None,
			quantity: None,
		})
		.await
		.unwrap();
	let output = run_cli(&server_a, &["pay", &offer_resp.offer, "10000sat"]);
	assert!(output.get("bolt12_payment_id").is_some());
}

#[tokio::test]
async fn test_cli_bolt12_send() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	// Create offer on B via client lib
	let offer_resp = server_b
		.client()
		.bolt12_receive(Bolt12ReceiveRequest {
			description: "test offer".to_string(),
			amount_msat: None,
			expiry_secs: None,
			quantity: None,
		})
		.await
		.unwrap();

	// Send via CLI from A
	let output = run_cli(&server_a, &["bolt12-send", &offer_resp.offer, "10000sat"]);
	assert!(!output["payment_id"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_cli_spontaneous_send() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;

	let mut consumer_a = RabbitMqEventConsumer::new(&server_a.exchange_name).await;
	let mut consumer_b = RabbitMqEventConsumer::new(&server_b.exchange_name).await;

	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	let node_id_hex = server_b.node_id().to_lower_hex_string();
	let output = run_cli(&server_a, &["spontaneous-send", &node_id_hex, "10000sat"]);
	assert!(!output["payment_id"].as_str().unwrap().is_empty());

	// Verify events
	tokio::time::sleep(Duration::from_secs(5)).await;

	let events_a = consumer_a.consume_events(5, Duration::from_secs(10)).await;
	assert!(
		events_a.iter().any(|e| matches!(e, Event::PaymentSuccessful(_))),
		"Expected PaymentSuccessful on sender"
	);

	let events_b = consumer_b.consume_events(5, Duration::from_secs(10)).await;
	assert!(
		events_b.iter().any(|e| matches!(e, Event::PaymentReceived(_))),
		"Expected PaymentReceived on receiver"
	);
}

#[tokio::test]
async fn test_cli_get_payment_details() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	// Make a bolt11 payment via CLI
	let invoice_resp = server_b
		.client()
		.bolt11_receive(Bolt11ReceiveRequest {
			amount_msat: Some(10_000_000),
			description: Some(Bolt11InvoiceDescription::Direct("test".to_string())),
			expiry_secs: 3600,
		})
		.await
		.unwrap();

	let invoice: Bolt11Invoice = invoice_resp.invoice.parse().unwrap();

	let send_output = run_cli(&server_a, &["bolt11-send", &invoice_resp.invoice]);
	let payment_id = send_output["payment_id"].as_str().unwrap();

	// Wait for payment to be recorded
	tokio::time::sleep(Duration::from_secs(3)).await;

	let output = run_cli(&server_a, &["get-payment-details", payment_id]);
	let payment = &output["payment"];
	assert_eq!(payment["id"], payment_id);
	assert_eq!(payment["status"], "succeeded");
	assert_eq!(payment["direction"], "outbound");

	// Verify the payment hash in the details matches the invoice
	let details_hash_hex = payment["kind"]["bolt11"]["hash"].as_str().unwrap();
	let details_hash = <[u8; 32]>::from_hex(details_hash_hex).unwrap();
	assert_eq!(
		details_hash,
		*invoice.payment_hash().as_byte_array(),
		"payment hash in details should match invoice"
	);
}

#[tokio::test]
async fn test_cli_list_payments() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	// Make a bolt11 payment via CLI
	let invoice_resp = server_b
		.client()
		.bolt11_receive(Bolt11ReceiveRequest {
			amount_msat: Some(10_000_000),
			description: Some(Bolt11InvoiceDescription::Direct("test".to_string())),
			expiry_secs: 3600,
		})
		.await
		.unwrap();

	run_cli(&server_a, &["bolt11-send", &invoice_resp.invoice]);
	tokio::time::sleep(Duration::from_secs(3)).await;

	let output = run_cli(&server_a, &["list-payments"]);
	assert!(!output["list"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_cli_close_channel() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	let user_channel_id = setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	let node_id_hex = server_b.node_id().to_lower_hex_string();
	let output = run_cli(&server_a, &["close-channel", &user_channel_id, &node_id_hex]);
	assert!(output.is_object());

	mine_and_sync(&bitcoind, &[&server_a, &server_b], 6).await;
	tokio::time::sleep(Duration::from_secs(2)).await;

	let channels_output = run_cli(&server_a, &["list-channels"]);
	assert!(channels_output["channels"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_cli_force_close_channel() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	let user_channel_id = setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	let node_id_hex = server_b.node_id().to_lower_hex_string();
	let output = run_cli(&server_a, &["force-close-channel", &user_channel_id, &node_id_hex]);
	assert!(output.is_object());

	mine_and_sync(&bitcoind, &[&server_a, &server_b], 6).await;
	tokio::time::sleep(Duration::from_secs(2)).await;

	let channels_output = run_cli(&server_a, &["list-channels"]);
	assert!(channels_output["channels"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_cli_splice_in() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	let user_channel_id = setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	let node_id_hex = server_b.node_id().to_lower_hex_string();
	let output = run_cli(&server_a, &["splice-in", &user_channel_id, &node_id_hex, "50000sat"]);
	assert!(output.is_object());
}

#[tokio::test]
async fn test_cli_splice_out() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	let user_channel_id = setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	let node_id_hex = server_b.node_id().to_lower_hex_string();
	let output = run_cli(&server_a, &["splice-out", &user_channel_id, &node_id_hex, "10000sat"]);
	let address = output["address"].as_str().unwrap();
	assert!(address.starts_with("bcrt1"), "Expected regtest address, got: {}", address);
}

#[tokio::test]
async fn test_cli_graph_list_channels_empty() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let output = run_cli(&server, &["graph-list-channels"]);
	assert!(output["short_channel_ids"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_cli_graph_list_nodes_empty() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let output = run_cli(&server, &["graph-list-nodes"]);
	assert!(output["node_ids"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_cli_graph_with_channel() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	// Wait for the channel announcement to appear in the network graph.
	let scid = {
		let start = std::time::Instant::now();
		loop {
			let output = run_cli(&server_a, &["graph-list-channels"]);
			let scids = output["short_channel_ids"].as_array().unwrap();
			if !scids.is_empty() {
				break scids[0].as_u64().unwrap().to_string();
			}
			if start.elapsed() > Duration::from_secs(30) {
				panic!("Timed out waiting for channel to appear in network graph");
			}
			tokio::time::sleep(Duration::from_secs(1)).await;
		}
	};

	// Test GraphGetChannel: should return channel info with both our nodes.
	let output = run_cli(&server_a, &["graph-get-channel", &scid]);
	let channel = &output["channel"];
	let node_one = channel["node_one"].as_str().unwrap();
	let node_two = channel["node_two"].as_str().unwrap();
	let node_a_hex = server_a.node_id().to_lower_hex_string();
	let node_b_hex = server_b.node_id().to_lower_hex_string();
	let nodes = [node_a_hex.as_str(), node_b_hex.as_str()];
	assert!(nodes.contains(&node_one), "node_one {} not one of our nodes", node_one);
	assert!(nodes.contains(&node_two), "node_two {} not one of our nodes", node_two);

	// Test GraphListNodes: should contain both node IDs.
	let output = run_cli(&server_a, &["graph-list-nodes"]);
	let node_ids: Vec<&str> =
		output["node_ids"].as_array().unwrap().iter().map(|n| n.as_str().unwrap()).collect();
	assert!(node_ids.contains(&node_a_hex.as_str()), "Expected server_a in graph nodes");
	assert!(node_ids.contains(&node_b_hex.as_str()), "Expected server_b in graph nodes");

	// Test GraphGetNode: should return node info with at least one channel.
	let output = run_cli(&server_a, &["graph-get-node", &node_b_hex]);
	let node = &output["node"];
	let channels = node["channels"].as_array().unwrap();
	assert!(!channels.is_empty(), "Expected node to have at least one channel");
}

#[tokio::test]
async fn test_cli_completions() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let output = run_cli_raw(&server, &["completions", "bash"]);
	assert!(!output.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_forwarded_payment_event() {
	let bitcoind = TestBitcoind::new();

	// A: normal payer node
	let server_a = LdkServerHandle::start(&bitcoind).await;

	// B: LSP node (all e2e servers include LSPS2 service config)
	let server_b = LdkServerHandle::start(&bitcoind).await;

	// Set up RabbitMQ consumer on B before any payments
	let mut consumer_b = RabbitMqEventConsumer::new(&server_b.exchange_name).await;

	// Open channel A -> B (1M sats, larger for JIT forwarding)
	setup_funded_channel(&bitcoind, &server_a, &server_b, 1_000_000).await;

	// Fund B additionally so it can open JIT channel to C
	let addr_b = server_b.client().onchain_receive(OnchainReceiveRequest {}).await.unwrap().address;
	bitcoind.fund_address(&addr_b, 1.0);
	mine_and_sync(&bitcoind, &[&server_a, &server_b], 6).await;

	// C: raw ldk-node configured as LSPS2 client of B
	#[allow(deprecated)]
	let storage_dir_c = tempfile::tempdir().unwrap().into_path();
	let p2p_port_c = find_available_port();
	let config_c = ldk_node::config::Config {
		network: Network::Regtest,
		storage_dir_path: storage_dir_c.to_str().unwrap().to_string(),
		listening_addresses: Some(vec![SocketAddress::from_str(&format!(
			"127.0.0.1:{p2p_port_c}"
		))
		.unwrap()]),
		..Default::default()
	};

	let mut builder_c = ldk_node::Builder::from_config(config_c);
	let (rpc_host, rpc_port, rpc_user, rpc_password) = bitcoind.rpc_details();
	builder_c.set_chain_source_bitcoind_rpc(rpc_host, rpc_port, rpc_user, rpc_password);

	// Set B as LSPS2 LSP for C
	let b_node_id_hex = server_b.node_id().to_lower_hex_string();
	let b_node_id = PublicKey::from_str(&b_node_id_hex).unwrap();
	let b_addr = SocketAddress::from_str(&format!("127.0.0.1:{}", server_b.p2p_port)).unwrap();
	builder_c.set_liquidity_source_lsps2(b_node_id, b_addr, None);

	let seed_path_c = storage_dir_c.join("keys_seed").to_str().unwrap().to_string();
	let node_entropy_c = ldk_node::entropy::NodeEntropy::from_seed_path(seed_path_c).unwrap();
	let node_c = builder_c.build(node_entropy_c).unwrap();

	node_c.start().unwrap();

	node_c.sync_wallets().unwrap();

	// C generates JIT invoice via LSPS2
	let description = ldk_node::lightning_invoice::Bolt11InvoiceDescription::Direct(
		ldk_node::lightning_invoice::Description::new("test jit".to_string()).unwrap(),
	);
	let jit_invoice = node_c
		.bolt11_payment()
		.receive_via_jit_channel(100_000_000, &description, 3600, None)
		.unwrap();

	// A pays the JIT invoice (routes through B)
	run_cli(&server_a, &["bolt11-send", &jit_invoice.to_string()]);

	// Wait for payment processing and JIT channel open
	tokio::time::sleep(Duration::from_secs(15)).await;

	// Mine blocks to confirm JIT channel
	mine_and_sync(&bitcoind, &[&server_a, &server_b], 6).await;
	tokio::time::sleep(Duration::from_secs(5)).await;

	// Verify PaymentForwarded event on B
	let events_b = consumer_b.consume_events(10, Duration::from_secs(15)).await;
	assert!(
		events_b.iter().any(|e| matches!(e, Event::PaymentForwarded(_))),
		"Expected PaymentForwarded event on LSP node B, got events: {:?}",
		events_b.iter().map(|e| e).collect::<Vec<_>>()
	);

	node_c.stop().unwrap();
}

#[tokio::test]
async fn test_hodl_invoice_claim() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;

	let mut consumer_a = RabbitMqEventConsumer::new(&server_a.exchange_name).await;
	let mut consumer_b = RabbitMqEventConsumer::new(&server_b.exchange_name).await;

	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	// Test three claim variants: (preimage, amount, hash)
	let test_cases: Vec<([u8; 32], Option<&str>, bool)> = vec![
		([42u8; 32], Some("10000000msat"), true),  // all args
		([44u8; 32], Some("10000000msat"), false), // preimage + amount
		([45u8; 32], None, true),                  // preimage + hash
		([46u8; 32], None, false),                 // preimage only
	];

	for (preimage_bytes, amount, include_hash) in &test_cases {
		let preimage_hex = preimage_bytes.to_lower_hex_string();
		let payment_hash_hex =
			sha256::Hash::hash(preimage_bytes).to_byte_array().to_lower_hex_string();

		// Create hodl invoice on B
		let invoice_resp = run_cli(
			&server_b,
			&[
				"bolt11-receive-for-hash",
				&payment_hash_hex,
				"10000000msat",
				"-d",
				"hodl test",
				"-e",
				"3600",
			],
		);
		let invoice = invoice_resp["invoice"].as_str().unwrap();

		// Pay the hodl invoice from A
		run_cli(&server_a, &["bolt11-send", invoice]);

		// Verify PaymentClaimable event on B
		let events_b = consumer_b.consume_events(1, Duration::from_secs(10)).await;
		assert!(
			events_b.iter().any(|e| matches!(e, Event::PaymentClaimable(_))),
			"Expected PaymentClaimable on receiver, got events: {:?}",
			events_b.iter().map(|e| e).collect::<Vec<_>>()
		);

		// Claim the payment on B
		let mut args: Vec<&str> = vec!["bolt11-claim-for-hash", &preimage_hex];
		if let Some(amt) = amount {
			args.extend(["-c", amt]);
		}
		if *include_hash {
			args.extend(["-p", &payment_hash_hex]);
		}
		run_cli(&server_b, &args);

		// Verify PaymentReceived event on B
		let events_b = consumer_b.consume_events(1, Duration::from_secs(10)).await;
		assert!(
			events_b.iter().any(|e| matches!(e, Event::PaymentReceived(_))),
			"Expected PaymentReceived on receiver after claim, got events: {:?}",
			events_b.iter().map(|e| e).collect::<Vec<_>>()
		);

		// Verify PaymentSuccessful on A
		let events_a = consumer_a.consume_events(1, Duration::from_secs(10)).await;
		assert!(
			events_a.iter().any(|e| matches!(e, Event::PaymentSuccessful(_))),
			"Expected PaymentSuccessful on sender, got events: {:?}",
			events_a.iter().map(|e| e).collect::<Vec<_>>()
		);
	}
}

#[tokio::test]
async fn test_hodl_invoice_fail() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;

	// Set up event consumers before any payments
	let mut consumer_a = RabbitMqEventConsumer::new(&server_a.exchange_name).await;
	let mut consumer_b = RabbitMqEventConsumer::new(&server_b.exchange_name).await;

	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	// Generate a known preimage and compute its payment hash
	let preimage_bytes = [43u8; 32];
	let payment_hash = sha256::Hash::hash(&preimage_bytes);
	let payment_hash_hex = payment_hash.to_byte_array().to_lower_hex_string();

	// Create hodl invoice on B
	let invoice_resp = run_cli(
		&server_b,
		&[
			"bolt11-receive-for-hash",
			&payment_hash_hex,
			"10000000msat",
			"-d",
			"hodl fail test",
			"-e",
			"3600",
		],
	);
	let invoice = invoice_resp["invoice"].as_str().unwrap();

	// Pay the hodl invoice from A
	run_cli(&server_a, &["bolt11-send", invoice]);

	// Wait for payment to arrive at B
	tokio::time::sleep(Duration::from_secs(5)).await;

	// Verify PaymentClaimable event on B
	let events_b = consumer_b.consume_events(5, Duration::from_secs(10)).await;
	assert!(
		events_b.iter().any(|e| matches!(e, Event::PaymentClaimable(_))),
		"Expected PaymentClaimable on receiver, got events: {:?}",
		events_b.iter().map(|e| e).collect::<Vec<_>>()
	);

	// Fail the payment on B using CLI
	run_cli(&server_b, &["bolt11-fail-for-hash", &payment_hash_hex]);

	// Wait for failure to propagate
	tokio::time::sleep(Duration::from_secs(5)).await;

	// Verify PaymentFailed on A
	let events_a = consumer_a.consume_events(10, Duration::from_secs(10)).await;
	assert!(
		events_a.iter().any(|e| matches!(e, Event::PaymentFailed(_))),
		"Expected PaymentFailed on sender after hodl rejection, got events: {:?}",
		events_a.iter().map(|e| e).collect::<Vec<_>>()
	);
}
