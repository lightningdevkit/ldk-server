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
	wait_for_onchain_balance, LdkServerConfig, LdkServerHandle, RabbitMqEventConsumer,
	TestBitcoind,
};
use hex_conservative::{DisplayHex, FromHex};
use ldk_node::bitcoin::hashes::{sha256, Hash};
use ldk_node::lightning::ln::msgs::SocketAddress;
use ldk_node::lightning::offers::offer::Offer;
use ldk_node::lightning_invoice::Bolt11Invoice;
use ldk_server_client::ldk_server_grpc::api::{
	Bolt11ReceiveRequest, Bolt12ReceiveRequest, OnchainReceiveRequest,
};
use ldk_server_client::ldk_server_grpc::types::{
	bolt11_invoice_description, Bolt11InvoiceDescription,
};
use ldk_server_grpc::events::event_envelope::Event;

#[tokio::test]
async fn test_cli_get_node_info() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let output = run_cli(&server, &["get-node-info"]);
	assert!(output.get("node_id").is_some());
	assert_eq!(output["node_id"], server.node_id());
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

	let output = run_cli(&server, &["verify-signature", "hello", signature, server.node_id()]);
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
			description: Some(Bolt11InvoiceDescription {
				kind: Some(bolt11_invoice_description::Kind::Direct("test".to_string())),
			}),
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
	let payment_hash = sha256::Hash::from_str(output["payment_hash"].as_str().unwrap()).unwrap();
	assert_eq!(invoice.payment_hash().0, payment_hash.to_byte_array());
	let payment_secret = <[u8; 32]>::from_hex(output["payment_secret"].as_str().unwrap()).unwrap();
	assert_eq!(invoice.payment_secret().0, payment_secret);
}

#[tokio::test]
async fn test_cli_decode_invoice() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	// Create a BOLT11 invoice with known parameters
	let output =
		run_cli(&server, &["bolt11-receive", "50000sat", "-d", "decode test", "-e", "3600"]);
	let invoice_str = output["invoice"].as_str().unwrap();

	// Decode it
	let decoded = run_cli(&server, &["decode-invoice", invoice_str]);

	// Verify fields match
	assert_eq!(decoded["destination"], server.node_id());
	assert_eq!(decoded["payment_hash"], output["payment_hash"]);
	assert_eq!(decoded["amount_msat"], 50_000_000);
	assert_eq!(decoded["description"], "decode test");
	assert!(decoded.get("description_hash").is_none() || decoded["description_hash"].is_null());
	assert_eq!(decoded["expiry"], 3600);
	assert_eq!(decoded["currency"], "regtest");
	assert_eq!(decoded["payment_secret"], output["payment_secret"]);
	assert!(decoded["timestamp"].as_u64().unwrap() > 0);
	assert!(decoded["min_final_cltv_expiry_delta"].as_u64().unwrap() > 0);
	assert_eq!(decoded["is_expired"], false);

	// Verify features — LDK BOLT11 invoices always set VariableLengthOnion, PaymentSecret,
	// and BasicMPP.
	let features = decoded["features"].as_object().unwrap();
	assert!(!features.is_empty(), "Expected at least one feature");

	let feature_names: Vec<&str> = features.values().filter_map(|f| f["name"].as_str()).collect();
	assert!(
		feature_names.contains(&"VariableLengthOnion"),
		"Expected VariableLengthOnion in features: {:?}",
		feature_names
	);
	assert!(
		feature_names.contains(&"PaymentSecret"),
		"Expected PaymentSecret in features: {:?}",
		feature_names
	);
	assert!(
		feature_names.contains(&"BasicMPP"),
		"Expected BasicMPP in features: {:?}",
		feature_names
	);

	// Every entry should have the expected structure
	for (bit, feature) in features {
		assert!(bit.parse::<u32>().is_ok(), "Feature key should be a bit number: {}", bit);
		assert!(feature.get("name").is_some(), "Feature missing name field");
		assert!(feature.get("is_required").is_some(), "Feature missing is_required field");
		assert!(feature.get("is_known").is_some(), "Feature missing is_known field");
	}

	// Also test a variable-amount invoice
	let output_var = run_cli(&server, &["bolt11-receive", "-d", "no amount"]);
	let decoded_var =
		run_cli(&server, &["decode-invoice", output_var["invoice"].as_str().unwrap()]);
	assert!(decoded_var.get("amount_msat").is_none() || decoded_var["amount_msat"].is_null());
	assert_eq!(decoded_var["description"], "no amount");

	// Test that ANSI escape sequences cannot reach the terminal via CLI output.
	// serde_json escapes control chars (U+0000–U+001F) as \uXXXX in JSON.
	let desc_with_ansi = "pay me\x1b[31m RED \x1b[0m";
	let output_ansi = run_cli(&server, &["bolt11-receive", "-d", desc_with_ansi]);
	let raw_decoded = run_cli_raw(
		&server,
		&["decode-invoice", output_ansi["invoice"].as_str().unwrap()],
	);
	assert!(
		!raw_decoded.contains('\x1b'),
		"Raw CLI output must not contain ANSI escape bytes"
	);

	// Test that Unicode bidi override characters in the description are escaped
	// (sanitize_for_terminal replaces them with \uXXXX in CLI output)
	let desc_with_bidi = "pay me\u{202E}evil";
	let output_bidi = run_cli(&server, &["bolt11-receive", "-d", desc_with_bidi]);
	let raw_bidi = run_cli_raw(
		&server,
		&["decode-invoice", output_bidi["invoice"].as_str().unwrap()],
	);
	assert!(
		!raw_bidi.contains('\u{202E}'),
		"Raw CLI output must not contain bidi override characters"
	);
	assert!(
		raw_bidi.contains("\\u202E"),
		"Bidi characters should be escaped as \\uXXXX in output"
	);
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
async fn test_cli_decode_offer() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	// BOLT12 offers need announced channels for blinded reply paths
	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	// Create a BOLT12 offer with known parameters
	let output = run_cli(&server_a, &["bolt12-receive", "decode offer test"]);
	let offer_str = output["offer"].as_str().unwrap();

	// Decode it
	let decoded = run_cli(&server_a, &["decode-offer", offer_str]);

	// Verify fields match
	assert_eq!(decoded["offer_id"], output["offer_id"]);
	assert_eq!(decoded["description"], "decode offer test");
	assert_eq!(decoded["is_expired"], false);

	// Chains should include regtest
	let chains = decoded["chains"].as_array().unwrap();
	assert!(chains.iter().any(|c| c == "regtest"), "Expected regtest in chains: {:?}", chains);

	// Paths should be present (BOLT12 offers with blinded paths)
	let paths = decoded["paths"].as_array().unwrap();
	assert!(!paths.is_empty(), "Expected at least one blinded path");
	for path in paths {
		assert!(path["num_hops"].as_u64().unwrap() > 0);
		assert!(!path["blinding_point"].as_str().unwrap().is_empty());
	}

	// Features — OfferContext has no known features in LDK, so this should be empty
	let features = decoded["features"].as_object().unwrap();
	assert!(features.is_empty(), "Expected empty offer features, got: {:?}", features);

	// Variable-amount offer should have no amount
	assert!(decoded.get("amount").is_none() || decoded["amount"].is_null());

	// Test a fixed-amount offer
	let output_fixed = run_cli(&server_a, &["bolt12-receive", "fixed amount", "50000sat"]);
	let decoded_fixed =
		run_cli(&server_a, &["decode-offer", output_fixed["offer"].as_str().unwrap()]);
	assert_eq!(decoded_fixed["amount"]["amount"]["bitcoin_amount_msats"], 50_000_000);

	// Test that ANSI escape sequences cannot reach the terminal via CLI output.
	let desc_with_ansi = "offer\x1b[31m RED \x1b[0m";
	let output_ansi = run_cli(&server_a, &["bolt12-receive", desc_with_ansi]);
	let raw_decoded = run_cli_raw(
		&server_a,
		&["decode-offer", output_ansi["offer"].as_str().unwrap()],
	);
	assert!(
		!raw_decoded.contains('\x1b'),
		"Raw CLI output must not contain ANSI escape bytes"
	);

	// Test that Unicode bidi override characters in the description are escaped
	let desc_with_bidi = "offer\u{202E}evil";
	let output_bidi = run_cli(&server_a, &["bolt12-receive", desc_with_bidi]);
	let raw_bidi = run_cli_raw(
		&server_a,
		&["decode-offer", output_bidi["offer"].as_str().unwrap()],
	);
	assert!(
		!raw_bidi.contains('\u{202E}'),
		"Raw CLI output must not contain bidi override characters"
	);
	assert!(
		raw_bidi.contains("\\u202E"),
		"Bidi characters should be escaped as \\uXXXX in output"
	);
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
	assert!(!output["txid"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_cli_connect_peer() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;

	let addr = format!("127.0.0.1:{}", server_b.p2p_port);
	let output = run_cli(&server_a, &["connect-peer", server_b.node_id(), &addr]);
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
	run_cli(&server_a, &["connect-peer", server_b.node_id(), &addr]);

	let output = run_cli(&server_a, &["list-peers"]);
	let peers = output["peers"].as_array().unwrap();
	assert_eq!(peers.len(), 1);
	assert_eq!(peers[0]["node_id"], server_b.node_id());
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
	let output = run_cli(
		&server_a,
		&["open-channel", server_b.node_id(), &addr, "100000sat", "--announce-channel"],
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
	assert_eq!(channels[0]["counterparty_node_id"], server_b.node_id());
}

#[tokio::test]
async fn test_cli_update_channel_config() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	let user_channel_id = setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	let output = run_cli(
		&server_a,
		&[
			"update-channel-config",
			&user_channel_id,
			server_b.node_id(),
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
			description: Some(Bolt11InvoiceDescription {
				kind: Some(bolt11_invoice_description::Kind::Direct("test".to_string())),
			}),
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
		events_a.iter().any(|e| matches!(&e.event, Some(Event::PaymentSuccessful(_)))),
		"Expected PaymentSuccessful on sender"
	);

	let events_b = consumer_b.consume_events(5, Duration::from_secs(10)).await;
	assert!(
		events_b.iter().any(|e| matches!(&e.event, Some(Event::PaymentReceived(_)))),
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
			description: Some(Bolt11InvoiceDescription {
				kind: Some(bolt11_invoice_description::Kind::Direct("test".to_string())),
			}),
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

	let output = run_cli(&server_a, &["spontaneous-send", server_b.node_id(), "10000sat"]);
	assert!(!output["payment_id"].as_str().unwrap().is_empty());

	// Verify events
	tokio::time::sleep(Duration::from_secs(5)).await;

	let events_a = consumer_a.consume_events(5, Duration::from_secs(10)).await;
	assert!(
		events_a.iter().any(|e| matches!(&e.event, Some(Event::PaymentSuccessful(_)))),
		"Expected PaymentSuccessful on sender"
	);

	let events_b = consumer_b.consume_events(5, Duration::from_secs(10)).await;
	assert!(
		events_b.iter().any(|e| matches!(&e.event, Some(Event::PaymentReceived(_)))),
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
			description: Some(Bolt11InvoiceDescription {
				kind: Some(bolt11_invoice_description::Kind::Direct("test".to_string())),
			}),
			expiry_secs: 3600,
		})
		.await
		.unwrap();

	let send_output = run_cli(&server_a, &["bolt11-send", &invoice_resp.invoice]);
	let payment_id = send_output["payment_id"].as_str().unwrap();

	// Wait for payment to be recorded
	tokio::time::sleep(Duration::from_secs(3)).await;

	let output = run_cli(&server_a, &["get-payment-details", payment_id]);
	assert!(output.get("payment").is_some());
	assert_eq!(output["payment"]["id"], payment_id);
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
			description: Some(Bolt11InvoiceDescription {
				kind: Some(bolt11_invoice_description::Kind::Direct("test".to_string())),
			}),
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

	let output = run_cli(&server_a, &["close-channel", &user_channel_id, server_b.node_id()]);
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

	let output = run_cli(&server_a, &["force-close-channel", &user_channel_id, server_b.node_id()]);
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

	let output =
		run_cli(&server_a, &["splice-in", &user_channel_id, server_b.node_id(), "50000sat"]);
	assert!(output.is_object());
}

#[tokio::test]
async fn test_cli_splice_out() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	let user_channel_id = setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	let output =
		run_cli(&server_a, &["splice-out", &user_channel_id, server_b.node_id(), "10000sat"]);
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
	let nodes = [server_a.node_id(), server_b.node_id()];
	assert!(nodes.contains(&node_one), "node_one {} not one of our nodes", node_one);
	assert!(nodes.contains(&node_two), "node_two {} not one of our nodes", node_two);

	// Test GraphListNodes: should contain both node IDs.
	let output = run_cli(&server_a, &["graph-list-nodes"]);
	let node_ids: Vec<&str> =
		output["node_ids"].as_array().unwrap().iter().map(|n| n.as_str().unwrap()).collect();
	assert!(node_ids.contains(&server_a.node_id()), "Expected server_a in graph nodes");
	assert!(node_ids.contains(&server_b.node_id()), "Expected server_b in graph nodes");

	// Test GraphGetNode: should return node info with at least one channel.
	let output = run_cli(&server_a, &["graph-get-node", server_b.node_id()]);
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
		network: ldk_node::bitcoin::Network::Regtest,
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
	let b_node_id = ldk_node::bitcoin::secp256k1::PublicKey::from_str(server_b.node_id()).unwrap();
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
		events_b.iter().any(|e| matches!(&e.event, Some(Event::PaymentForwarded(_)))),
		"Expected PaymentForwarded event on LSP node B, got events: {:?}",
		events_b.iter().map(|e| &e.event).collect::<Vec<_>>()
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
			events_b.iter().any(|e| matches!(&e.event, Some(Event::PaymentClaimable(_)))),
			"Expected PaymentClaimable on receiver, got events: {:?}",
			events_b.iter().map(|e| &e.event).collect::<Vec<_>>()
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
			events_b.iter().any(|e| matches!(&e.event, Some(Event::PaymentReceived(_)))),
			"Expected PaymentReceived on receiver after claim, got events: {:?}",
			events_b.iter().map(|e| &e.event).collect::<Vec<_>>()
		);

		// Verify PaymentSuccessful on A
		let events_a = consumer_a.consume_events(1, Duration::from_secs(10)).await;
		assert!(
			events_a.iter().any(|e| matches!(&e.event, Some(Event::PaymentSuccessful(_)))),
			"Expected PaymentSuccessful on sender, got events: {:?}",
			events_a.iter().map(|e| &e.event).collect::<Vec<_>>()
		);
	}
}

#[tokio::test]
async fn test_hodl_invoice_fail() {
	use hex_conservative::DisplayHex;
	use ldk_node::bitcoin::hashes::{sha256, Hash};

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
		events_b.iter().any(|e| matches!(&e.event, Some(Event::PaymentClaimable(_)))),
		"Expected PaymentClaimable on receiver, got events: {:?}",
		events_b.iter().map(|e| &e.event).collect::<Vec<_>>()
	);

	// Fail the payment on B using CLI
	run_cli(&server_b, &["bolt11-fail-for-hash", &payment_hash_hex]);

	// Wait for failure to propagate
	tokio::time::sleep(Duration::from_secs(5)).await;

	// Verify PaymentFailed on A
	let events_a = consumer_a.consume_events(10, Duration::from_secs(10)).await;
	assert!(
		events_a.iter().any(|e| matches!(&e.event, Some(Event::PaymentFailed(_)))),
		"Expected PaymentFailed on sender after hodl rejection, got events: {:?}",
		events_a.iter().map(|e| &e.event).collect::<Vec<_>>()
	);
}

#[tokio::test]
async fn test_metrics_endpoint() {
	let bitcoind = TestBitcoind::new();

	// Test with metrics enabled
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;

	let client = server_a.client();
	let metrics_result = client.get_metrics().await;

	assert!(metrics_result.is_ok(), "Expected metrics to succeed when enabled");
	let metrics = metrics_result.unwrap();

	// Verify initial state
	assert!(metrics.contains("ldk_server_total_peers_count 0"));
	assert!(metrics.contains("ldk_server_total_payments_count 0"));
	assert!(metrics.contains("ldk_server_total_successful_payments_count 0"));
	assert!(metrics.contains("ldk_server_total_pending_payments_count 0"));
	assert!(metrics.contains("ldk_server_total_failed_payments_count 0"));
	assert!(metrics.contains("ldk_server_total_channels_count 0"));
	assert!(metrics.contains("ldk_server_total_public_channels_count 0"));
	assert!(metrics.contains("ldk_server_total_private_channels_count 0"));
	assert!(metrics.contains("ldk_server_total_onchain_balance_sats 0"));
	assert!(metrics.contains("ldk_server_spendable_onchain_balance_sats 0"));
	assert!(metrics.contains("ldk_server_total_anchor_channels_reserve_sats 0"));
	assert!(metrics.contains("ldk_server_total_lightning_balance_sats 0"));

	// Set up channel and make a payment to trigger metrics update
	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	// Poll for channel, peer and balance metrics.
	let timeout = Duration::from_secs(10);
	let start = std::time::Instant::now();
	loop {
		let metrics = client.get_metrics().await.unwrap();
		if metrics.contains("ldk_server_total_peers_count 1")
			&& metrics.contains("ldk_server_total_channels_count 1")
			&& metrics.contains("ldk_server_total_public_channels_count 1")
			&& metrics.contains("ldk_server_total_payments_count 2")
			&& !metrics.contains("ldk_server_total_lightning_balance_sats 0")
			&& !metrics.contains("ldk_server_total_onchain_balance_sats 0")
			&& !metrics.contains("ldk_server_spendable_onchain_balance_sats 0")
			&& !metrics.contains("ldk_server_total_anchor_channels_reserve_sats 0")
		{
			break;
		}

		if start.elapsed() > timeout {
			let current_metrics = client.get_metrics().await.unwrap();
			panic!(
				"Timed out waiting for channel, peer and balance metrics to update. Current metrics:\n{}",
				current_metrics
			);
		}
		tokio::time::sleep(Duration::from_secs(1)).await;
	}

	let invoice_resp = server_b
		.client()
		.bolt11_receive(Bolt11ReceiveRequest {
			amount_msat: Some(10_000_000),
			description: Some(Bolt11InvoiceDescription {
				kind: Some(bolt11_invoice_description::Kind::Direct("metrics test".to_string())),
			}),
			expiry_secs: 3600,
		})
		.await
		.unwrap();

	run_cli(&server_a, &["bolt11-send", &invoice_resp.invoice]);

	// Wait to receive the PaymentSuccessful event and update metrics
	let timeout = Duration::from_secs(30);
	let start = std::time::Instant::now();
	loop {
		let metrics = client.get_metrics().await.unwrap();
		if metrics.contains("ldk_server_total_successful_payments_count 1")
			&& !metrics.contains("ldk_server_total_lightning_balance_sats 0")
			&& !metrics.contains("ldk_server_total_onchain_balance_sats 0")
			&& !metrics.contains("ldk_server_spendable_onchain_balance_sats 0")
			&& !metrics.contains("ldk_server_total_anchor_channels_reserve_sats 0")
		{
			break;
		}
		if start.elapsed() > timeout {
			panic!("Timed out waiting for payment metrics to update");
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	}
}

#[tokio::test]
async fn test_metrics_endpoint_with_auth() {
	let bitcoind = TestBitcoind::new();

	let username = "admin";
	let password = "password123";

	let config =
		LdkServerConfig { metrics_auth: Some((username.to_string(), password.to_string())) };

	let server = LdkServerHandle::start_with_config(&bitcoind, config).await;
	let client = server.client();

	// Should fail because auth is provided in the config
	let result = client.get_metrics().await;
	assert!(result.is_err(), "Expected failure without credentials");

	// Request has the correct auth, so it should succeed
	let result = client.get_metrics_with_auth(Some(username), Some(password)).await;

	assert!(result.is_ok(), "Expected success with correct credentials");
	let metrics = result.unwrap();

	assert!(metrics.contains("ldk_server_total_peers_count 0"));
	assert!(metrics.contains("ldk_server_total_payments_count 0"));
	assert!(metrics.contains("ldk_server_total_successful_payments_count 0"));
	assert!(metrics.contains("ldk_server_total_pending_payments_count 0"));
	assert!(metrics.contains("ldk_server_total_failed_payments_count 0"));
	assert!(metrics.contains("ldk_server_total_channels_count 0"));
	assert!(metrics.contains("ldk_server_total_public_channels_count 0"));
	assert!(metrics.contains("ldk_server_total_private_channels_count 0"));
	assert!(metrics.contains("ldk_server_total_onchain_balance_sats 0"));
	assert!(metrics.contains("ldk_server_spendable_onchain_balance_sats 0"));
	assert!(metrics.contains("ldk_server_total_anchor_channels_reserve_sats 0"));
	assert!(metrics.contains("ldk_server_total_lightning_balance_sats 0"));
}
