// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use e2e_tests::{
	start_expect_failure, ChainSource, LdkServerHandle, TestBitcoind, TestConfigBuilder,
};
use ldk_server_grpc::api::GetNodeInfoRequest;

#[tokio::test]
async fn test_config_no_alias() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		TestConfigBuilder::new(params).alias(None).build()
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
	assert!(info.node_alias.is_none());
}

#[tokio::test]
async fn test_config_no_listening_addresses() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		// Alias requires listening addresses for announcement, so drop it too.
		TestConfigBuilder::new(params).listening_addresses(vec![]).alias(None).build()
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
	assert!(info.node_alias.is_none());
	assert!(info.listening_addresses.is_empty());
}

#[tokio::test]
async fn test_config_multiple_listening_addresses() {
	let bitcoind = TestBitcoind::new();
	let extra_port = e2e_tests::find_available_port();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		TestConfigBuilder::new(params)
			.listening_addresses(vec![
				format!("127.0.0.1:{}", params.p2p_port),
				format!("127.0.0.1:{}", extra_port),
			])
			.build()
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
	assert!(info.listening_addresses.contains(&format!("127.0.0.1:{}", server.p2p_port)));
	assert!(info.listening_addresses.contains(&format!("127.0.0.1:{}", extra_port)));
}

#[tokio::test]
async fn test_config_with_announcement_addresses() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		TestConfigBuilder::new(params)
			.announcement_addresses(vec![format!("127.0.0.1:{}", params.p2p_port)])
			.build()
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
	assert_eq!(info.node_alias, Some(String::from("e2e-test-node")));
	assert!(info.listening_addresses.contains(&format!("127.0.0.1:{}", server.p2p_port)));
	assert!(info.announcement_addresses.contains(&format!("127.0.0.1:{}", server.p2p_port)));
}

#[tokio::test]
async fn test_config_with_log_file() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		let log_path = format!("{}/ldk-server.log", params.storage_dir.display());
		TestConfigBuilder::new(params).log(Some("Debug"), &log_path).build()
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_with_tls_hosts() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		TestConfigBuilder::new(params)
			.tls_hosts(vec!["example.com".to_string(), "ldk-server.local".to_string()])
			.build()
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[test]
fn test_config_fail_log_file_matches_storage_dir() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		let storage_dir = params.storage_dir.display().to_string();
		TestConfigBuilder::new(params).log(None, &storage_dir).build()
	});
	assert!(
		stderr.contains("Log file path cannot be the same as storage directory path"),
		"Unexpected stderr: {stderr}"
	);
}

#[test]
fn test_config_fail_log_file_matches_network_dir() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		let network_dir = params.storage_dir.join("regtest").display().to_string();
		TestConfigBuilder::new(params).log(None, &network_dir).build()
	});
	assert!(
		stderr.contains("Log file path cannot be the same as storage directory path"),
		"Unexpected stderr: {stderr}"
	);
}

#[tokio::test]
async fn test_config_chain_source_bitcoind_localhost() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		// Use "localhost:port" instead of "127.0.0.1:port" to test hostname RPC support
		let rpc_address = params.rpc_address.replace("127.0.0.1", "localhost");
		TestConfigBuilder::new(params)
			.chain_source(ChainSource::Bitcoind {
				rpc_address,
				rpc_user: params.rpc_user.clone(),
				rpc_password: params.rpc_password.clone(),
			})
			.build()
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}
