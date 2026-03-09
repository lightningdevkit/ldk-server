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
	create_restricted_client, find_available_port, make_client, mine_and_sync, run_cli,
	run_cli_raw, setup_funded_channel, wait_for_onchain_balance, LdkServerHandle,
	RabbitMqEventConsumer, TestBitcoind,
};
use ldk_node::lightning::ln::msgs::SocketAddress;
use ldk_server_client::ldk_server_protos::api::{
	Bolt11ReceiveRequest, Bolt12ReceiveRequest, GetNodeInfoRequest, GetPermissionsRequest,
	OnchainReceiveRequest,
};
use ldk_server_client::ldk_server_protos::types::{
	bolt11_invoice_description, Bolt11InvoiceDescription,
};
use ldk_server_protos::events::event_envelope::Event;

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
	let invoice = output["invoice"].as_str().unwrap();
	assert!(invoice.starts_with("lnbcrt"), "Expected lnbcrt prefix, got: {}", invoice);
}

#[tokio::test]
async fn test_cli_bolt12_receive() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	// BOLT12 offers need announced channels for blinded reply paths
	setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;

	let output = run_cli(&server_a, &["bolt12-receive", "test offer"]);
	let offer = output["offer"].as_str().unwrap();
	assert!(offer.starts_with("lno"), "Expected lno prefix, got: {}", offer);
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
	let consumer_a = RabbitMqEventConsumer::new(&server_a.exchange_name).await;
	let consumer_b = RabbitMqEventConsumer::new(&server_b.exchange_name).await;

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

	let consumer_a = RabbitMqEventConsumer::new(&server_a.exchange_name).await;
	let consumer_b = RabbitMqEventConsumer::new(&server_b.exchange_name).await;

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
	let consumer_b = RabbitMqEventConsumer::new(&server_b.exchange_name).await;

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
async fn test_get_permissions_admin() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let resp = server.client().get_permissions(GetPermissionsRequest {}).await.unwrap();
	assert!(
		resp.endpoints.contains(&"*".to_string()),
		"Expected admin key to have wildcard permission"
	);
}

#[tokio::test]
async fn test_create_api_key_and_get_permissions() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let restricted_client = create_restricted_client(
		&server,
		"read-only",
		vec!["GetNodeInfo".to_string(), "GetBalances".to_string()],
	)
	.await;

	let resp = restricted_client.get_permissions(GetPermissionsRequest {}).await.unwrap();
	assert_eq!(resp.endpoints, vec!["GetBalances", "GetNodeInfo"]);
}

#[tokio::test]
async fn test_restricted_key_allowed_endpoint() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let restricted_client =
		create_restricted_client(&server, "node-info-only", vec!["GetNodeInfo".to_string()]).await;

	let resp = restricted_client.get_node_info(GetNodeInfoRequest {}).await;
	assert!(resp.is_ok(), "Restricted key should be able to call allowed endpoint");
	assert_eq!(resp.unwrap().node_id, server.node_id());
}

#[tokio::test]
async fn test_restricted_key_denied_endpoint() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let restricted_client =
		create_restricted_client(&server, "info-only", vec!["GetNodeInfo".to_string()]).await;

	let resp = restricted_client.onchain_receive(OnchainReceiveRequest {}).await;
	assert!(resp.is_err(), "Restricted key should be denied access to unauthorized endpoint");
}

#[tokio::test]
async fn test_restricted_key_get_permissions_always_allowed() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	// Create key with no endpoints at all (except GetPermissions which is always allowed)
	let restricted_client =
		create_restricted_client(&server, "perms-only", vec!["GetNodeInfo".to_string()]).await;

	let resp = restricted_client.get_permissions(GetPermissionsRequest {}).await;
	assert!(resp.is_ok(), "GetPermissions should always be allowed");
}

#[tokio::test]
async fn test_create_api_key_via_cli() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let output =
		run_cli(&server, &["create-api-key", "cli-test-key", "-e", "GetNodeInfo", "GetBalances"]);
	let api_key = output["api_key"].as_str().unwrap();
	assert_eq!(api_key.len(), 64);
	assert!(api_key.chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test]
async fn test_invalid_api_key_rejected() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let bad_key = "ff".repeat(32);
	let bad_client = make_client(&server, &bad_key);

	let resp = bad_client.get_node_info(GetNodeInfoRequest {}).await;
	assert!(resp.is_err(), "Invalid API key should be rejected");
}

#[tokio::test]
async fn test_restricted_key_cannot_create_api_key() {
	use ldk_server_client::ldk_server_protos::api::CreateApiKeyRequest;

	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let restricted = create_restricted_client(&server, "limited", vec!["GetNodeInfo".to_string()])
		.await;

	// Restricted key should not be able to create new keys
	let result = restricted
		.create_api_key(CreateApiKeyRequest {
			name: "sneaky".to_string(),
			endpoints: vec!["*".to_string()],
		})
		.await;
	assert!(result.is_err(), "Restricted key should not be able to create API keys");
	assert_eq!(
		result.unwrap_err().error_code,
		ldk_server_client::error::LdkServerErrorCode::AuthError
	);
}

#[tokio::test]
async fn test_create_api_key_duplicate_name_rejected() {
	use ldk_server_client::ldk_server_protos::api::CreateApiKeyRequest;

	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	// First creation should succeed
	let result = server
		.client()
		.create_api_key(CreateApiKeyRequest {
			name: "my-key".to_string(),
			endpoints: vec!["GetNodeInfo".to_string()],
		})
		.await;
	assert!(result.is_ok());

	// Duplicate name should fail
	let result = server
		.client()
		.create_api_key(CreateApiKeyRequest {
			name: "my-key".to_string(),
			endpoints: vec!["GetNodeInfo".to_string()],
		})
		.await;
	assert!(result.is_err(), "Duplicate API key name should be rejected");
}

#[tokio::test]
async fn test_create_api_key_invalid_endpoint_rejected() {
	use ldk_server_client::ldk_server_protos::api::CreateApiKeyRequest;

	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;

	let result = server
		.client()
		.create_api_key(CreateApiKeyRequest {
			name: "bad-key".to_string(),
			endpoints: vec!["NonExistentEndpoint".to_string()],
		})
		.await;
	assert!(result.is_err(), "Unknown endpoint should be rejected");
}
