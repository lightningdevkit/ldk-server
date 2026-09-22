// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::time::Duration;

use e2e_tests::{
	assert_recovered_balance, close_channel, expected_onchain_balance, list_payments,
	mine_and_sync, send_bolt11_payment, setup_funded_channel, wait_for_channels,
	wait_for_force_close_claims, wait_for_forwarded_payments, wait_for_gossip,
	wait_for_settled_balance, wait_for_usable_channel, LdkServerHandle, TestBitcoind,
	TestConfigBuilder,
};
use ldk_server_grpc::api::{
	open_channel_request, ConnectPeerRequest, DisconnectPeerRequest, ForceCloseChannelRequest,
	GetBalancesRequest, GetNodeInfoRequest, ListChannelForwardingStatsRequest,
	OnchainReceiveRequest, OpenChannelRequest,
};
use ldk_server_grpc::types::{lightning_balance, payment_kind, BalanceSource, PaymentStatus};

const TIMEOUT: Duration = Duration::from_secs(60);

async fn start_postgres(bitcoind: &TestBitcoind, connection_string: &str) -> LdkServerHandle {
	let server = LdkServerHandle::start_with_config(bitcoind, |params| {
		// Each server gets its own table, even when sharing the same test database.
		let table_name = format!("node_{}", params.grpc_port);
		TestConfigBuilder::new(params)
			.postgres(connection_string, &table_name)
			.forwarded_payment_tracking_mode("detailed")
			.log(Some("Info"), params.storage_dir.join("ldk-server.log").to_str().unwrap())
			.build()
	})
	.await;
	assert!(server.storage_dir.join("regtest/ldk_node_postgres.lock").exists());
	assert!(!server.storage_dir.join("regtest/ldk_node_data.sqlite").exists());
	server
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "requires PostgreSQL; set POSTGRES_CONNECTION_STRING"]
async fn test_postgres_table_isolation() {
	let connection_string = std::env::var("POSTGRES_CONNECTION_STRING")
		.expect("Set POSTGRES_CONNECTION_STRING to a disposable PostgreSQL database");
	let bitcoind = TestBitcoind::new();
	// Both processes must run concurrently. Ignoring kv_table_name would make the second
	// process contend for the first process's PostgreSQL store lock.
	let mut servers = [
		start_postgres(&bitcoind, &connection_string).await,
		start_postgres(&bitcoind, &connection_string).await,
	];
	assert_ne!(servers[0].node_id(), servers[1].node_id());

	let address_a = servers[0].client().onchain_receive(OnchainReceiveRequest {}).await.unwrap();
	bitcoind.fund_address(&address_a.address, 0.01);
	mine_and_sync(&bitcoind, &[&servers[0], &servers[1]], 6).await;
	wait_for_settled_balance(&servers[0], 1_000_000, TIMEOUT).await;
	wait_for_settled_balance(&servers[1], 0, TIMEOUT).await;
	assert!(list_payments(&servers[1]).await.is_empty());

	let address_b = servers[1].client().onchain_receive(OnchainReceiveRequest {}).await.unwrap();
	assert_ne!(address_a.address, address_b.address);
	bitcoind.fund_address(&address_b.address, 0.02);
	mine_and_sync(&bitcoind, &[&servers[0], &servers[1]], 6).await;
	for (server, expected_sats) in servers.iter_mut().zip([1_000_000, 2_000_000]) {
		// Wait for the funding payment to be confirmed before comparing persisted records.
		assert_eq!(
			expected_onchain_balance(&bitcoind, server, 0, &[], 1, TIMEOUT).await,
			expected_sats
		);
		wait_for_settled_balance(server, expected_sats, TIMEOUT).await;
		let saved_payments = list_payments(server).await;
		server.restart().await;
		wait_for_settled_balance(server, expected_sats, TIMEOUT).await;
		assert_eq!(list_payments(server).await, saved_payments);
	}
	wait_for_settled_balance(&servers[0], 1_000_000, TIMEOUT).await;
	wait_for_settled_balance(&servers[1], 2_000_000, TIMEOUT).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "requires PostgreSQL; set POSTGRES_CONNECTION_STRING"]
async fn test_postgres_persistence_and_sqlite_interoperability() {
	let connection_string = std::env::var("POSTGRES_CONNECTION_STRING")
		.expect("Set POSTGRES_CONNECTION_STRING to a disposable PostgreSQL database");
	let bitcoind = TestBitcoind::new();
	let sqlite_a = LdkServerHandle::start(&bitcoind).await;
	let mut postgres = start_postgres(&bitcoind, &connection_string).await;
	let sqlite_c = LdkServerHandle::start(&bitcoind).await;
	let first_address = postgres.client().onchain_receive(OnchainReceiveRequest {}).await.unwrap();

	// SQLite A -> PostgreSQL B -> SQLite C. B both accepts and initiates a channel.
	let channel_ab = setup_funded_channel(&bitcoind, &sqlite_a, &postgres, 1_000_000).await;
	let channel_bc = setup_funded_channel(&bitcoind, &postgres, &sqlite_c, 1_000_000).await;
	// The shared helper waits for any usable channel on the funder; B already has A-B.
	// Keep mining until C's only channel is confirmed, too.
	wait_for_usable_channel(sqlite_c.client(), &bitcoind, TIMEOUT).await;
	wait_for_channels(&sqlite_a, 1, TIMEOUT).await;
	wait_for_channels(&postgres, 2, TIMEOUT).await;
	wait_for_channels(&sqlite_c, 1, TIMEOUT).await;
	wait_for_gossip(&sqlite_a, 2, TIMEOUT).await;
	wait_for_gossip(&sqlite_c, 2, TIMEOUT).await;

	// Check that the test really selected different backends.
	for sqlite in [&sqlite_a, &sqlite_c] {
		assert!(sqlite.storage_dir.join("regtest/ldk_node_data.sqlite").exists());
		assert!(!sqlite.storage_dir.join("regtest/ldk_node_postgres.lock").exists());
	}

	send_bolt11_payment(&sqlite_a, &postgres, 50_000_000).await;
	send_bolt11_payment(&postgres, &sqlite_a, 10_000_000).await;
	send_bolt11_payment(&postgres, &sqlite_c, 50_000_000).await;
	send_bolt11_payment(&sqlite_c, &postgres, 10_000_000).await;
	send_bolt11_payment(&sqlite_a, &sqlite_c, 50_000_000).await;
	send_bolt11_payment(&sqlite_c, &sqlite_a, 10_000_000).await;
	let saved_forwards = wait_for_forwarded_payments(&postgres, 2, TIMEOUT).await;
	for (from, to) in [(&sqlite_a, &sqlite_c), (&sqlite_c, &sqlite_a)] {
		assert!(saved_forwards.iter().any(|p| {
			p.prev_node_id.as_deref() == Some(from.node_id())
				&& p.next_node_id.as_deref() == Some(to.node_id())
		}));
	}
	let saved_payments = list_payments(&postgres).await;
	assert_eq!(
		saved_payments
			.iter()
			.filter(|p| {
				p.status == PaymentStatus::Succeeded as i32
					&& matches!(
						p.kind.as_ref().and_then(|kind| kind.kind.as_ref()),
						Some(payment_kind::Kind::Bolt11(_))
					)
			})
			.count(),
		4
	);
	let saved_channels = wait_for_channels(&postgres, 2, TIMEOUT).await;
	let saved_balances = postgres.client().get_balances(GetBalancesRequest {}).await.unwrap();
	assert!(saved_balances.total_onchain_balance_sats > 0);
	assert!(saved_balances.total_lightning_balance_sats > 0);
	let saved_stats = postgres
		.client()
		.list_channel_forwarding_stats(ListChannelForwardingStatsRequest { page_token: None })
		.await
		.unwrap();
	assert_eq!(saved_stats.stats.len(), 2);
	let address_before = postgres.client().onchain_receive(OnchainReceiveRequest {}).await.unwrap();

	// A process kill tests committed state without relying on a graceful shutdown flush.
	postgres.restart().await;
	// A initiated A-B, so reconnect it without waiting for its periodic reconnect timer.
	sqlite_a
		.client()
		.connect_peer(ConnectPeerRequest {
			node_pubkey: postgres.node_id().to_string(),
			address: format!("127.0.0.1:{}", postgres.p2p_port),
			persist: true,
		})
		.await
		.unwrap();
	let restored_channels = wait_for_channels(&postgres, 2, TIMEOUT).await;
	for saved in saved_channels {
		let restored = restored_channels.iter().find(|c| c.channel_id == saved.channel_id).unwrap();
		assert_eq!(restored.user_channel_id, saved.user_channel_id);
		assert_eq!(restored.counterparty_node_id, saved.counterparty_node_id);
		assert_eq!(restored.funding_txo, saved.funding_txo);
		assert_eq!(restored.channel_value_sats, saved.channel_value_sats);
	}
	let restored_balances = postgres.client().get_balances(GetBalancesRequest {}).await.unwrap();
	assert_eq!(
		restored_balances.total_onchain_balance_sats,
		saved_balances.total_onchain_balance_sats
	);
	assert_eq!(
		restored_balances.total_lightning_balance_sats,
		saved_balances.total_lightning_balance_sats
	);
	assert_eq!(list_payments(&postgres).await, saved_payments);
	assert_eq!(wait_for_forwarded_payments(&postgres, 2, TIMEOUT).await, saved_forwards);
	let restored_stats = postgres
		.client()
		.list_channel_forwarding_stats(ListChannelForwardingStatsRequest { page_token: None })
		.await
		.unwrap();
	assert_eq!(restored_stats, saved_stats);
	let address_after = postgres.client().onchain_receive(OnchainReceiveRequest {}).await.unwrap();
	assert_ne!(first_address.address, address_after.address);
	assert_ne!(address_before.address, address_after.address);

	// Existing channels must still carry HTLCs in both directions after recovery.
	wait_for_channels(&sqlite_a, 1, TIMEOUT).await;
	wait_for_channels(&sqlite_c, 1, TIMEOUT).await;
	wait_for_gossip(&sqlite_a, 2, TIMEOUT).await;
	wait_for_gossip(&sqlite_c, 2, TIMEOUT).await;
	send_bolt11_payment(&sqlite_a, &sqlite_c, 20_000_000).await;
	send_bolt11_payment(&sqlite_c, &sqlite_a, 5_000_000).await;
	wait_for_forwarded_payments(&postgres, 4, TIMEOUT).await;

	let mut before_close = Vec::new();
	for server in [&sqlite_a, &postgres, &sqlite_c] {
		let balances = server.client().get_balances(GetBalancesRequest {}).await.unwrap();
		before_close.push((balances, list_payments(server).await));
	}

	// Close from each backend, and check both endpoints removed their channels.
	close_channel(&sqlite_a, &postgres, &channel_ab).await;
	close_channel(&postgres, &sqlite_c, &channel_bc).await;
	for server in [&sqlite_a, &postgres, &sqlite_c] {
		wait_for_channels(server, 0, TIMEOUT).await;
	}
	mine_and_sync(&bitcoind, &[&sqlite_a, &postgres, &sqlite_c], 6).await;
	let mut expected_cooperative_balances = Vec::new();
	for ((server, receipts), (balances_before, payments_before)) in
		[(&sqlite_a, 1), (&postgres, 2), (&sqlite_c, 1)].iter().zip(&before_close)
	{
		let expected = expected_onchain_balance(
			&bitcoind,
			server,
			balances_before.total_onchain_balance_sats,
			payments_before,
			*receipts,
			TIMEOUT,
		)
		.await;
		let recovered = wait_for_settled_balance(server, expected, TIMEOUT).await;
		assert_recovered_balance(balances_before, &recovered);
		expected_cooperative_balances.push(expected);
	}
	postgres.restart().await;
	wait_for_channels(&postgres, 0, TIMEOUT).await;
	wait_for_forwarded_payments(&postgres, 4, TIMEOUT).await;
	wait_for_settled_balance(&postgres, expected_cooperative_balances[1], TIMEOUT).await;

	// Put funds on both sides at channel creation so no HTLC settlement races with force-close.
	let force_channel = postgres
		.client()
		.open_channel(OpenChannelRequest {
			node_pubkey: sqlite_c.node_id().to_string(),
			address: format!("127.0.0.1:{}", sqlite_c.p2p_port),
			amount: Some(open_channel_request::Amount::ChannelAmountSats(1_000_000)),
			push_to_counterparty_msat: Some(50_000_000),
			channel_config: None,
			announce_channel: false,
			disable_counterparty_reserve: false,
		})
		.await
		.unwrap()
		.user_channel_id;
	wait_for_usable_channel(postgres.client(), &bitcoind, TIMEOUT).await;
	wait_for_channels(&sqlite_c, 1, TIMEOUT).await;
	let mut before_force_close = Vec::new();
	for server in [&postgres, &sqlite_c] {
		let balances = server.client().get_balances(GetBalancesRequest {}).await.unwrap();
		before_force_close.push((balances, list_payments(server).await));
	}
	// Have SQLite discover the unilateral close onchain, ensuring PostgreSQL's commitment
	// confirms without a competing commitment broadcast in response to a peer error message.
	// Clear both peer stores so neither side reconnects before the commitment confirms.
	postgres
		.client()
		.disconnect_peer(DisconnectPeerRequest { node_pubkey: sqlite_c.node_id().to_string() })
		.await
		.unwrap();
	sqlite_c
		.client()
		.disconnect_peer(DisconnectPeerRequest { node_pubkey: postgres.node_id().to_string() })
		.await
		.unwrap();
	postgres
		.client()
		.force_close_channel(ForceCloseChannelRequest {
			user_channel_id: force_channel,
			counterparty_node_id: sqlite_c.node_id().to_string(),
			force_close_reason: Some("PostgreSQL persistence e2e test".to_string()),
		})
		.await
		.unwrap();
	let force_claims = wait_for_force_close_claims(
		&bitcoind,
		&[
			(&postgres, BalanceSource::HolderForceClosed),
			(&sqlite_c, BalanceSource::CounterpartyForceClosed),
		],
		TIMEOUT,
	)
	.await;
	wait_for_channels(&postgres, 0, TIMEOUT).await;
	wait_for_channels(&sqlite_c, 0, TIMEOUT).await;

	// Recover persisted channel monitors while the force-close outputs are still timelocked.
	postgres.restart().await;
	let restored = postgres.client().get_balances(GetBalancesRequest {}).await.unwrap();
	assert_eq!(restored.lightning_balances.len(), 1);
	assert_eq!(
		restored.lightning_balances[0].balance_type,
		Some(lightning_balance::BalanceType::ClaimableAwaitingConfirmations(
			force_claims[0].clone()
		))
	);
	let maturity_height =
		force_claims.iter().map(|claim| u64::from(claim.confirmation_height)).max().unwrap();
	let height = postgres
		.client()
		.get_node_info(GetNodeInfoRequest {})
		.await
		.unwrap()
		.current_best_block
		.unwrap()
		.height as u64;
	mine_and_sync(&bitcoind, &[&postgres, &sqlite_c], maturity_height - height).await;

	// Confirm the sweeps and reconcile every wallet credit/debit, including any anchor-bump
	// fees, using only ldk-server's public payment and balance APIs.
	let mut expected_force_balances = Vec::new();
	for (server, (balances_before, payments_before)) in
		[&postgres, &sqlite_c].iter().zip(&before_force_close)
	{
		let expected = expected_onchain_balance(
			&bitcoind,
			server,
			balances_before.total_onchain_balance_sats,
			payments_before,
			1,
			TIMEOUT,
		)
		.await;
		let recovered = wait_for_settled_balance(server, expected, TIMEOUT).await;
		assert_recovered_balance(balances_before, &recovered);
		expected_force_balances.push(expected);
	}
	wait_for_settled_balance(&sqlite_a, expected_cooperative_balances[0], TIMEOUT).await;
	let closed_payments = list_payments(&postgres).await;
	postgres.restart().await;
	wait_for_channels(&postgres, 0, TIMEOUT).await;
	wait_for_forwarded_payments(&postgres, 4, TIMEOUT).await;
	wait_for_settled_balance(&postgres, expected_force_balances[0], TIMEOUT).await;
	assert_eq!(list_payments(&postgres).await, closed_payments);
}
