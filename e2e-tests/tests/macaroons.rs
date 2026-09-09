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
	mine_and_sync, run_cli, setup_funded_channel, wait_for_event, LdkServerHandle, TestBitcoind,
};
use ldk_server_client::client::LdkServerClient;
use ldk_server_client::error::LdkServerErrorCode::{
	AuthError, AuthorizationError, InvalidRequestError,
};
use ldk_server_client::macaroon::{derive_macaroon, Macaroon};
use ldk_server_grpc::api::{GetNodeInfoRequest, GetPermissionsRequest, OnchainReceiveRequest};
use ldk_server_grpc::events::event_envelope::Event;
use ldk_server_grpc::events::ChannelState;

fn client_with_macaroon(server: &LdkServerHandle, token: impl Into<String>) -> LdkServerClient {
	let certificate = std::fs::read(&server.tls_cert_path).unwrap();
	LdkServerClient::new(server.base_url(), token.into(), &certificate).unwrap()
}

#[tokio::test]
async fn test_scoped_macaroon_lifecycle() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		let log_path = params.storage_dir.join("macaroon-audit.log");
		e2e_tests::TestConfigBuilder::new(params)
			.log(Some("Info"), log_path.to_str().unwrap())
			.build()
	})
	.await;

	let created = run_cli(&server, &["create-macaroon", "readonly-client", "--preset", "readonly"]);
	let root_id = created["macaroon"]["id"].as_str().unwrap();
	let secret = created["token"].as_str().unwrap();
	let client = client_with_macaroon(&server, secret.to_string());

	client.get_node_info(GetNodeInfoRequest {}).await.unwrap();
	let permissions = client.get_permissions(GetPermissionsRequest {}).await.unwrap();
	let info = permissions.macaroon.unwrap();
	assert_eq!(info.name, "readonly-client");
	assert!(info.caveats.iter().any(|c| c.starts_with("permissions = ")));
	assert_eq!(
		client.onchain_receive(OnchainReceiveRequest {}).await.unwrap_err().error_code,
		AuthorizationError
	);
	assert_eq!(
		client.list_macaroons(Default::default()).await.unwrap_err().error_code,
		AuthorizationError
	);

	let roots = run_cli(&server, &["list-macaroons"]);
	assert!(roots["macaroons"].as_array().unwrap().iter().any(|info| info["id"] == root_id));
	// Offline derivation must affect both unary and streaming authorization.
	let derived = ldk_server_client::macaroon::derive_macaroon(
		secret,
		&["permissions = node:read".into(), "method = GetNodeInfo".into()],
	)
	.unwrap();
	let restricted = client_with_macaroon(&server, derived.clone());
	let root_file = std::fs::read_to_string(
		server.storage_dir.join(format!("regtest/macaroons/roots/{root_id}.toml")),
	)
	.unwrap();
	let root_secret =
		root_file.lines().find_map(|line| line.strip_prefix("key = ")).unwrap().trim_matches('"');
	run_cli(&server, &["revoke-macaroon", &root_id.to_ascii_uppercase()]);
	let audit = std::fs::read_to_string(server.storage_dir.join("macaroon-audit.log")).unwrap();
	let issuer =
		server.client().get_permissions(Default::default()).await.unwrap().macaroon.unwrap().id;
	for action in ["Created", "Revoked"] {
		assert!(audit.contains(&format!(
			"{action} macaroon: issuer={issuer} id={root_id} name=readonly-client permissions="
		)));
	}
	assert!(!audit.contains(secret), "Audit log must not contain the token");
	assert!(!audit.contains(root_secret), "Audit log must not contain the root secret");
	assert_eq!(
		restricted.get_node_info(Default::default()).await.unwrap_err().error_code,
		AuthError
	);

	assert_eq!(
		client.subscribe_events().await.err().expect("Revoked root must not subscribe").error_code,
		AuthError
	);
	assert_eq!(
		client.get_node_info(GetNodeInfoRequest {}).await.unwrap_err().error_code,
		AuthError
	);
}

#[tokio::test]
async fn test_macaroon_splice_permissions() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;
	// Invalid request fields distinguish reaching the splice handler from an auth rejection.
	for (name, permission, expected) in [
		("manager", "channels:manage", AuthorizationError),
		("splicer", "channels:splice", InvalidRequestError),
	] {
		let created = run_cli(&server, &["create-macaroon", name, "--permissions", permission]);
		let scoped_client =
			client_with_macaroon(&server, created["token"].as_str().unwrap().to_string());
		assert_eq!(
			scoped_client.splice_in(Default::default()).await.unwrap_err().error_code,
			expected
		);
		assert_eq!(
			scoped_client.splice_out(Default::default()).await.unwrap_err().error_code,
			expected
		);
	}
}

#[tokio::test]
async fn test_macaroon_restrictions() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;
	let created = run_cli(&server, &["create-macaroon", "reader", "--preset", "readonly"]);
	let secret = created["token"].as_str().unwrap();
	// Offline derivation must affect both unary and streaming authorization.
	let derived = ldk_server_client::macaroon::derive_macaroon(
		secret,
		&["permissions = node:read".into(), "method = GetNodeInfo".into()],
	)
	.unwrap();
	let restricted = client_with_macaroon(&server, derived.clone());
	restricted.get_node_info(Default::default()).await.unwrap();
	assert_eq!(
		restricted.get_balances(Default::default()).await.unwrap_err().error_code,
		AuthorizationError
	);
	assert_eq!(restricted.subscribe_events().await.err().unwrap().error_code, AuthorizationError);
}

#[tokio::test]
async fn test_macaroon_expiry() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;
	let created = run_cli(&server, &["create-macaroon", "reader", "--preset", "readonly"]);
	let secret = created["token"].as_str().unwrap();
	let expired =
		ldk_server_client::macaroon::derive_macaroon(secret, &["time-before = 0".into()]).unwrap();
	let expired = client_with_macaroon(&server, expired);
	assert_eq!(
		expired.get_node_info(Default::default()).await.unwrap_err().error_code,
		AuthorizationError
	);

	let expiry = std::time::SystemTime::now() + std::time::Duration::from_secs(3);
	let expiry_seconds = expiry.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
	let expiry_caveat = format!("time-before = {expiry_seconds}");
	let expiring =
		ldk_server_client::macaroon::derive_macaroon(secret, std::slice::from_ref(&expiry_caveat))
			.unwrap();
	let expiring = client_with_macaroon(&server, expiring.to_ascii_uppercase());
	expiring.get_node_info(Default::default()).await.unwrap();
	assert!(expiring
		.get_permissions(Default::default())
		.await
		.unwrap()
		.macaroon
		.unwrap()
		.caveats
		.contains(&expiry_caveat));
	let events = expiring.subscribe_events().await.unwrap();
	tokio::time::sleep(expiry.duration_since(std::time::SystemTime::now()).unwrap_or_default())
		.await;
	assert_eq!(
		expiring.get_node_info(Default::default()).await.unwrap_err().error_code,
		AuthorizationError
	);
	assert_eq!(expiring.subscribe_events().await.err().unwrap().error_code, AuthorizationError);
	drop(events);
}

#[tokio::test]
async fn test_macaroon_delegation() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start(&bitcoind).await;
	// Request proofs must not become policy on newly issued credentials.
	let manager = run_cli(
		&server,
		&[
			"create-macaroon",
			"delegating-manager",
			"--permissions",
			"macaroons:manage",
			"node:read",
		],
	);
	let manager_expiry = format!(
		"time-before = {}",
		(std::time::SystemTime::now() + Duration::from_secs(3600))
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_secs()
	);
	let manager_token = ldk_server_client::macaroon::derive_macaroon(
		manager["token"].as_str().unwrap(),
		std::slice::from_ref(&manager_expiry),
	)
	.unwrap();
	let manager_client = client_with_macaroon(&server, manager_token);
	let child = manager_client
		.create_macaroon(ldk_server_grpc::api::CreateMacaroonRequest {
			name: "delegated-manager".into(),
			permissions: vec!["macaroons:manage".into(), "node:read".into()],
		})
		.await
		.unwrap();
	let child_client = client_with_macaroon(&server, child.token);
	let grandchild = child_client
		.create_macaroon(ldk_server_grpc::api::CreateMacaroonRequest {
			name: "delegated-reader".into(),
			permissions: vec!["node:read".into()],
		})
		.await
		.unwrap();
	let grandchild_client = client_with_macaroon(&server, grandchild.token);
	grandchild_client.get_node_info(Default::default()).await.unwrap();
	let info =
		grandchild_client.get_permissions(Default::default()).await.unwrap().macaroon.unwrap();
	assert!(info.caveats.contains(&manager_expiry));
	assert!(!info.caveats.iter().any(|c| c.starts_with("request = ")));
	assert!(!info.caveats.iter().any(|c| c == "method = CreateMacaroon"));
	assert_eq!(
		grandchild_client.onchain_receive(Default::default()).await.unwrap_err().error_code,
		AuthorizationError
	);
}

#[test]
fn test_offline_macaroon_derivation() {
	let token = Macaroon::mint(b"test root", b"test-id").unwrap().to_hex();
	let secret = token.as_str();
	let derived =
		derive_macaroon(secret, &["permissions = node:read".into(), "method = GetNodeInfo".into()])
			.unwrap();
	let output = std::process::Command::new(e2e_tests::cli_binary_path())
		.args([
			"--base-url",
			"invalid.invalid:1",
			"--tls-cert",
			"/no-certificate-needed",
			"derive-macaroon",
			secret,
			"--caveat",
			"permissions = node:read",
			"--caveat",
			"method = GetNodeInfo",
		])
		.output()
		.unwrap();
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), derived);
}

#[tokio::test]
async fn test_revoking_a_root_keeps_existing_event_streams_open() {
	let bitcoind = TestBitcoind::new();
	let server_a = LdkServerHandle::start(&bitcoind).await;
	let server_b = LdkServerHandle::start(&bitcoind).await;
	let channel_id = setup_funded_channel(&bitcoind, &server_a, &server_b, 100_000).await;
	let created =
		run_cli(&server_a, &["create-macaroon", "reader", "--permissions", "events:read"]);
	let client = client_with_macaroon(&server_a, created["token"].as_str().unwrap().to_string());
	let mut events = client.subscribe_events().await.unwrap();

	run_cli(&server_a, &["revoke-macaroon", created["macaroon"]["id"].as_str().unwrap()]);
	assert_eq!(
		client.subscribe_events().await.err().expect("Revoked root must not subscribe").error_code,
		AuthError
	);

	// An event created after revocation must still reach the existing subscription.
	run_cli(&server_a, &["close-channel", &channel_id, server_b.node_id()]);
	mine_and_sync(&bitcoind, &[&server_a, &server_b], 6).await;
	wait_for_event(&mut events, |event| {
		matches!(
			event,
			Event::ChannelStateChanged(channel_event)
				if channel_event.user_channel_id == channel_id
					&& channel_event.state == ChannelState::Closed as i32
		)
	})
	.await;
}
