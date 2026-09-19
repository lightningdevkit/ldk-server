// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::time::{Duration, Instant};

use e2e_tests::{
	mine_and_sync, setup_funded_channel, wait_for_event, wait_for_usable_channel, LdkServerHandle,
	TestBitcoind, TestConfigBuilder,
};
use ldk_server_client::error::LdkServerErrorCode;
use ldk_server_grpc::api::{
	open_channel_request, Bolt11ReceiveRequest, Bolt11SendRequest, CloseChannelRequest,
	ConnectPeerRequest, DisconnectPeerRequest, ForceCloseChannelRequest, GetBalancesRequest,
	GetBalancesResponse, GetNodeInfoRequest, GetPaymentDetailsRequest, GraphGetChannelRequest,
	GraphListChannelsRequest, ListChannelForwardingStatsRequest, ListChannelsRequest,
	ListForwardedPaymentsRequest, ListPaymentsRequest, OnchainReceiveRequest, OpenChannelRequest,
};
use ldk_server_grpc::events::event_envelope::Event;
use ldk_server_grpc::types::{
	lightning_balance, payment_kind, pending_sweep_balance, BalanceSource, Channel,
	ClaimableAwaitingConfirmations, ForwardedPayment, LightningBalance, Payment, PaymentDirection,
	PaymentStatus,
};

const TIMEOUT: Duration = Duration::from_secs(60);

async fn start_postgres(bitcoind: &TestBitcoind, connection_string: &str) -> LdkServerHandle {
	let server = LdkServerHandle::start_with_config(bitcoind, |params| {
		// Each server gets its own table, even when sharing the same test database.
		let table_name = format!(
			"node_{}",
			params.storage_dir.file_name().unwrap().to_str().unwrap().trim_start_matches('.')
		);
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

async fn channels(server: &LdkServerHandle, count: usize) -> Vec<Channel> {
	let start = Instant::now();
	loop {
		let channels =
			server.client().list_channels(ListChannelsRequest {}).await.unwrap().channels;
		if channels.len() == count && channels.iter().all(|c| c.is_usable) {
			return channels;
		}
		assert!(start.elapsed() < TIMEOUT, "Waiting for {count} usable channels: {channels:?}");
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
}

async fn close_channel(initiator: &LdkServerHandle, peer: &LdkServerHandle, user_channel_id: &str) {
	let start = Instant::now();
	loop {
		let result = initiator
			.client()
			.close_channel(CloseChannelRequest {
				user_channel_id: user_channel_id.to_string(),
				counterparty_node_id: peer.node_id().to_string(),
			})
			.await;
		match result {
			Ok(_) => return,
			Err(error) => {
				// The last HTLC's asynchronous monitor update can briefly block shutdown,
				// even after PaymentSuccessful and PaymentForwarded have been emitted.
				assert_eq!(error.error_code, LdkServerErrorCode::LightningError);
				assert!(start.elapsed() < TIMEOUT, "Channel closure failed: {error:?}");
				tokio::time::sleep(Duration::from_millis(200)).await;
			},
		}
	}
}

async fn wait_for_gossip(server: &LdkServerHandle) {
	let start = Instant::now();
	loop {
		let graph = server.client().graph_list_channels(GraphListChannelsRequest {}).await.unwrap();
		let mut ready = graph.short_channel_ids.len() == 2;
		for short_channel_id in graph.short_channel_ids {
			let channel = server
				.client()
				.graph_get_channel(GraphGetChannelRequest { short_channel_id })
				.await
				.unwrap()
				.channel
				.unwrap();
			ready &= channel.one_to_two.is_some_and(|update| update.enabled)
				&& channel.two_to_one.is_some_and(|update| update.enabled);
		}
		if ready {
			return;
		}
		assert!(start.elapsed() < TIMEOUT, "Timed out waiting for both channel announcements");
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
}

async fn pay(sender: &LdkServerHandle, receiver: &LdkServerHandle, amount_msat: u64) {
	let mut sent = sender.client().subscribe_events().await.unwrap();
	let mut received = receiver.client().subscribe_events().await.unwrap();
	let invoice = receiver
		.client()
		.bolt11_receive(Bolt11ReceiveRequest {
			amount_msat: Some(amount_msat),
			description: None,
			expiry_secs: 3600,
		})
		.await
		.unwrap();
	let payment_id = sender
		.client()
		.bolt11_send(Bolt11SendRequest {
			invoice: invoice.invoice,
			amount_msat: None,
			route_parameters: None,
		})
		.await
		.unwrap()
		.payment_id;
	wait_for_event(&mut sent, |event| {
		matches!(event, Event::PaymentSuccessful(e) if e.payment.as_ref().is_some_and(|p| p.payment_id == payment_id))
	})
	.await;
	let received =
		wait_for_event(&mut received, |event| matches!(event, Event::PaymentReceived(_))).await;
	let Some(Event::PaymentReceived(received)) = received.event else {
		panic!("Expected a PaymentReceived event after paying the BOLT11 invoice");
	};
	// Payment IDs are local to each node; correlate the two records by invoice hash.
	for (server, payment_id) in [(sender, payment_id), (receiver, received.payment_id)] {
		let payment = server
			.client()
			.get_payment_details(GetPaymentDetailsRequest { payment_id })
			.await
			.unwrap()
			.payment
			.unwrap();
		assert_eq!(payment.status, PaymentStatus::Succeeded as i32);
		assert_eq!(payment.amount_msat, Some(amount_msat));
		let Some(payment_kind::Kind::Bolt11(details)) = payment.kind.unwrap().kind else {
			panic!("Expected a BOLT11 payment");
		};
		assert_eq!(details.hash, invoice.payment_hash);
	}
}

async fn payments(server: &LdkServerHandle) -> Vec<Payment> {
	let response =
		server.client().list_payments(ListPaymentsRequest { page_token: None }).await.unwrap();
	assert!(response.next_page_token.is_none());
	response.payments
}

async fn forwards(server: &LdkServerHandle, count: usize) -> Vec<ForwardedPayment> {
	let start = Instant::now();
	loop {
		let response = server
			.client()
			.list_forwarded_payments(ListForwardedPaymentsRequest { page_token: None })
			.await
			.unwrap();
		assert!(response.next_page_token.is_none());
		if response.forwarded_payments.len() == count {
			return response.forwarded_payments;
		}
		assert!(start.elapsed() < TIMEOUT, "Expected {count} forwards: {response:?}");
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
}

// Observe the confirmed closing outputs before the channel monitors hand them to the wallet.
async fn force_close_claims(
	bitcoind: &TestBitcoind, servers: &[(&LdkServerHandle, BalanceSource)],
) -> Vec<ClaimableAwaitingConfirmations> {
	let handles: Vec<_> = servers.iter().map(|(server, _)| *server).collect();
	let start = Instant::now();
	loop {
		mine_and_sync(bitcoind, &handles, 1).await;
		let mut balances = Vec::new();
		for (server, _) in servers {
			balances.push(server.client().get_balances(GetBalancesRequest {}).await.unwrap());
		}
		let claims: Option<Vec<_>> = balances
			.iter()
			.zip(servers)
			.map(|(balances, (_, source))| match balances.lightning_balances.as_slice() {
				[LightningBalance {
					balance_type:
						Some(lightning_balance::BalanceType::ClaimableAwaitingConfirmations(claim)),
				}] if claim.source == *source as i32 => Some(claim.clone()),
				_ => None,
			})
			.collect();
		if let Some(claims) = claims {
			return claims;
		}
		assert!(start.elapsed() < TIMEOUT, "Waiting for confirmed closing outputs: {balances:?}");
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
}

async fn assert_settled_balance(
	server: &LdkServerHandle, expected_sats: u64,
) -> GetBalancesResponse {
	let start = Instant::now();
	loop {
		let balances = server.client().get_balances(GetBalancesRequest {}).await.unwrap();
		if balances.total_onchain_balance_sats == expected_sats
			&& balances.spendable_onchain_balance_sats == expected_sats
			&& balances.total_anchor_channels_reserve_sats == 0
			&& balances.total_lightning_balance_sats == 0
			&& balances.lightning_balances.is_empty()
			&& balances.pending_balances_from_channel_closures.iter().all(|balance| {
				matches!(
					balance.balance_type,
					Some(pending_sweep_balance::BalanceType::AwaitingThresholdConfirmations(_))
				)
			}) {
			return balances;
		}
		assert!(
			start.elapsed() < TIMEOUT,
			"Expected {} to recover {expected_sats} sats onchain: {balances:?}",
			server.node_id()
		);
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
}

// Compare funds before and after closure independently of the closing receipts. Allow for
// sweep fees and differences from the commitment fee already deducted from Lightning balances:
// a cheaper cooperative close can make the final onchain balance slightly higher.
fn assert_recovered_balance(before: &GetBalancesResponse, after: &GetBalancesResponse) {
	let before_sats = before.total_onchain_balance_sats + before.total_lightning_balance_sats;
	let after_sats = after.total_onchain_balance_sats;
	let fee_allowance_sats = 5_000;
	assert!(
		before_sats.abs_diff(after_sats) <= fee_allowance_sats,
		"Expected recovery of {before_sats} sats within {fee_allowance_sats} sats for fees, got {after_sats}"
	);
}

// Reconcile the wallet against the separately persisted onchain payment records. Incoming
// amounts are already net of fees; outgoing amounts exclude their separately reported fees.
async fn expected_onchain_balance(
	bitcoind: &TestBitcoind, server: &LdkServerHandle, balance_before: u64,
	payments_before: &[Payment], expected_receipts: usize,
) -> u64 {
	let start = Instant::now();
	loop {
		let new_payments: Vec<_> = payments(server)
			.await
			.into_iter()
			.filter(|payment| {
				matches!(
					payment.kind.as_ref().and_then(|kind| kind.kind.as_ref()),
					Some(payment_kind::Kind::Onchain(_))
				) && !payments_before.iter().any(|old| old.payment_id == payment.payment_id)
			})
			.collect();
		let receipts = new_payments
			.iter()
			.filter(|payment| payment.direction == PaymentDirection::Inbound as i32)
			.count();
		if receipts == expected_receipts
			&& new_payments.iter().all(|payment| payment.status == PaymentStatus::Succeeded as i32)
		{
			let mut expected_msat = i128::from(balance_before) * 1000;
			for payment in new_payments {
				let amount =
					i128::from(payment.amount_msat.expect("Missing onchain payment amount"));
				let direction = PaymentDirection::from_i32(payment.direction)
					.expect("Unexpected onchain payment direction");
				match direction {
					PaymentDirection::Inbound => expected_msat += amount,
					PaymentDirection::Outbound => {
						expected_msat -= amount
							+ i128::from(
								payment.fee_paid_msat.expect("Missing onchain transaction fee"),
							)
					},
				}
			}
			assert_eq!(expected_msat % 1000, 0);
			return u64::try_from(expected_msat / 1000).unwrap();
		}
		assert!(
			start.elapsed() < TIMEOUT,
			"Waiting for {expected_receipts} confirmed closing payments: {new_payments:?}"
		);
		// Broadcast and wallet sync are asynchronous, so keep confirming until the public API
		// reports the closing/sweep payments as succeeded.
		mine_and_sync(bitcoind, &[server], 1).await;
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
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
	assert_settled_balance(&servers[0], 1_000_000).await;
	assert_settled_balance(&servers[1], 0).await;
	assert!(payments(&servers[1]).await.is_empty());

	let address_b = servers[1].client().onchain_receive(OnchainReceiveRequest {}).await.unwrap();
	assert_ne!(address_a.address, address_b.address);
	bitcoind.fund_address(&address_b.address, 0.02);
	mine_and_sync(&bitcoind, &[&servers[0], &servers[1]], 6).await;
	for (server, expected_sats) in servers.iter_mut().zip([1_000_000, 2_000_000]) {
		// Wait for the funding payment to be confirmed before comparing persisted records.
		assert_eq!(expected_onchain_balance(&bitcoind, server, 0, &[], 1).await, expected_sats);
		assert_settled_balance(server, expected_sats).await;
		let saved_payments = payments(server).await;
		server.restart().await;
		assert_settled_balance(server, expected_sats).await;
		assert_eq!(payments(server).await, saved_payments);
	}
	assert_settled_balance(&servers[0], 1_000_000).await;
	assert_settled_balance(&servers[1], 2_000_000).await;
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
	channels(&sqlite_a, 1).await;
	channels(&postgres, 2).await;
	channels(&sqlite_c, 1).await;
	wait_for_gossip(&sqlite_a).await;
	wait_for_gossip(&sqlite_c).await;

	// Check that the test really selected different backends.
	for sqlite in [&sqlite_a, &sqlite_c] {
		assert!(sqlite.storage_dir.join("regtest/ldk_node_data.sqlite").exists());
		assert!(!sqlite.storage_dir.join("regtest/ldk_node_postgres.lock").exists());
	}

	pay(&sqlite_a, &postgres, 50_000_000).await;
	pay(&postgres, &sqlite_a, 10_000_000).await;
	pay(&postgres, &sqlite_c, 50_000_000).await;
	pay(&sqlite_c, &postgres, 10_000_000).await;
	pay(&sqlite_a, &sqlite_c, 50_000_000).await;
	pay(&sqlite_c, &sqlite_a, 10_000_000).await;
	let saved_forwards = forwards(&postgres, 2).await;
	for (from, to) in [(&sqlite_a, &sqlite_c), (&sqlite_c, &sqlite_a)] {
		assert!(saved_forwards.iter().any(|p| {
			p.prev_node_id.as_deref() == Some(from.node_id())
				&& p.next_node_id.as_deref() == Some(to.node_id())
		}));
	}
	let saved_payments = payments(&postgres).await;
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
	let saved_channels = channels(&postgres, 2).await;
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
	let restored_channels = channels(&postgres, 2).await;
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
	assert_eq!(payments(&postgres).await, saved_payments);
	assert_eq!(forwards(&postgres, 2).await, saved_forwards);
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
	channels(&sqlite_a, 1).await;
	channels(&sqlite_c, 1).await;
	wait_for_gossip(&sqlite_a).await;
	wait_for_gossip(&sqlite_c).await;
	pay(&sqlite_a, &sqlite_c, 20_000_000).await;
	pay(&sqlite_c, &sqlite_a, 5_000_000).await;
	forwards(&postgres, 4).await;

	let mut before_close = Vec::new();
	for server in [&sqlite_a, &postgres, &sqlite_c] {
		let balances = server.client().get_balances(GetBalancesRequest {}).await.unwrap();
		before_close.push((balances, payments(server).await));
	}

	// Close from each backend, and check both endpoints removed their channels.
	close_channel(&sqlite_a, &postgres, &channel_ab).await;
	close_channel(&postgres, &sqlite_c, &channel_bc).await;
	for server in [&sqlite_a, &postgres, &sqlite_c] {
		channels(server, 0).await;
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
		)
		.await;
		let recovered = assert_settled_balance(server, expected).await;
		assert_recovered_balance(balances_before, &recovered);
		expected_cooperative_balances.push(expected);
	}
	postgres.restart().await;
	channels(&postgres, 0).await;
	forwards(&postgres, 4).await;
	assert_settled_balance(&postgres, expected_cooperative_balances[1]).await;

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
	channels(&sqlite_c, 1).await;
	let mut before_force_close = Vec::new();
	for server in [&postgres, &sqlite_c] {
		let balances = server.client().get_balances(GetBalancesRequest {}).await.unwrap();
		before_force_close.push((balances, payments(server).await));
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
	let force_claims = force_close_claims(
		&bitcoind,
		&[
			(&postgres, BalanceSource::HolderForceClosed),
			(&sqlite_c, BalanceSource::CounterpartyForceClosed),
		],
	)
	.await;
	channels(&postgres, 0).await;
	channels(&sqlite_c, 0).await;

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
		)
		.await;
		let recovered = assert_settled_balance(server, expected).await;
		assert_recovered_balance(balances_before, &recovered);
		expected_force_balances.push(expected);
	}
	assert_settled_balance(&sqlite_a, expected_cooperative_balances[0]).await;
	let closed_payments = payments(&postgres).await;
	postgres.restart().await;
	channels(&postgres, 0).await;
	forwards(&postgres, 4).await;
	assert_settled_balance(&postgres, expected_force_balances[0]).await;
	assert_eq!(payments(&postgres).await, closed_payments);
}
