// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use e2e_tests::{
	default_test_config, start_expect_failure, test_config_with_chain_source, LdkServerHandle,
	TestBitcoind, TestElectrs,
};
use ldk_server_protos::api::GetNodeInfoRequest;

fn remove_config_line(config: &str, key: &str) -> String {
	config.lines().filter(|line| !line.trim_start().starts_with(key)).collect::<Vec<_>>().join("\n")
}

fn replace_config_line(config: &str, key: &str, new_line: &str) -> String {
	config
		.lines()
		.map(|line| if line.trim_start().starts_with(key) { new_line } else { line })
		.collect::<Vec<_>>()
		.join("\n")
}

fn remove_config_section(config: &str, section_header: &str) -> String {
	let mut result = Vec::new();
	let mut skipping = false;
	for line in config.lines() {
		let trimmed = line.trim();
		if trimmed == section_header {
			skipping = true;
			continue;
		}
		if skipping && trimmed.starts_with('[') {
			skipping = false;
		}
		if !skipping {
			result.push(line);
		}
	}
	result.join("\n")
}

#[tokio::test]
async fn test_config_no_alias() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		remove_config_line(&default_test_config(params), "alias =")
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_no_listening_addresses() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		let config = remove_config_line(&default_test_config(params), "listening_addresses =");
		// Alias requires listening addresses for announcement, so remove it too
		remove_config_line(&config, "alias =")
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_multiple_listening_addresses() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		let extra_port = e2e_tests::find_available_port();
		replace_config_line(
			&default_test_config(params),
			"listening_addresses =",
			&format!(
				"listening_addresses = [\"127.0.0.1:{}\", \"127.0.0.1:{}\"]",
				params.p2p_port, extra_port
			),
		)
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_with_announcement_addresses() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		let mut config = default_test_config(params);
		// Insert announcement_addresses after alias line
		config = config.replace(
			"alias = \"e2e-test-node\"",
			&format!(
				"alias = \"e2e-test-node\"\nannouncement_addresses = [\"127.0.0.1:{}\"]",
				params.p2p_port
			),
		);
		config
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_log_level_trace() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		let mut config = default_test_config(params);
		config.push_str("\n[log]\nlevel = \"Trace\"\n");
		config
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_log_level_error() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		let mut config = default_test_config(params);
		config.push_str("\n[log]\nlevel = \"Error\"\n");
		config
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_log_level_warn() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		let mut config = default_test_config(params);
		config.push_str("\n[log]\nlevel = \"Warn\"\n");
		config
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_with_log_file() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		let log_path = format!("{}/ldk-server.log", params.storage_dir.display());
		let mut config = default_test_config(params);
		config.push_str(&format!("\n[log]\nlevel = \"Debug\"\nfile = \"{}\"\n", log_path));
		config
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_with_tls_hosts() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		let mut config = default_test_config(params);
		config.push_str("\n[tls]\nhosts = [\"example.com\", \"ldk-server.local\"]\n");
		config
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_lsps2_advertise_service() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		replace_config_line(
			&default_test_config(params),
			"advertise_service =",
			"advertise_service = true",
		)
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_lsps2_with_require_token() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		let mut config = default_test_config(params);
		config = config.replace(
			"client_trusts_lsp = true",
			"client_trusts_lsp = true\nrequire_token = \"secret-token-123\"",
		);
		config
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_lsps2_high_fees() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		let config = default_test_config(params);
		let config = replace_config_line(
			&config,
			"channel_opening_fee_ppm =",
			"channel_opening_fee_ppm = 50000",
		);
		let config = replace_config_line(
			&config,
			"min_channel_opening_fee_msat =",
			"min_channel_opening_fee_msat = 10000000",
		);
		let config = replace_config_line(
			&config,
			"channel_over_provisioning_ppm =",
			"channel_over_provisioning_ppm = 500000",
		);
		config
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_lsps2_restrictive_limits() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		let config = default_test_config(params);
		let config = replace_config_line(
			&config,
			"min_payment_size_msat =",
			"min_payment_size_msat = 10000000",
		);
		let config = replace_config_line(
			&config,
			"max_payment_size_msat =",
			"max_payment_size_msat = 100000000",
		);
		let config =
			replace_config_line(&config, "min_channel_lifetime =", "min_channel_lifetime = 4320");
		let config = replace_config_line(
			&config,
			"max_client_to_self_delay =",
			"max_client_to_self_delay = 256",
		);
		config
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_lsps2_client_trusts_lsp_false() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		replace_config_line(
			&default_test_config(params),
			"client_trusts_lsp =",
			"client_trusts_lsp = false",
		)
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[test]
fn test_config_fail_missing_network() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		remove_config_line(&default_test_config(params), "network =")
	});
	assert!(stderr.contains("Missing `network`"), "Unexpected stderr: {stderr}");
}

#[test]
fn test_config_fail_missing_rest_service_address() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		remove_config_line(&default_test_config(params), "rest_service_address =")
	});
	assert!(stderr.contains("Missing `rest_service_address`"), "Unexpected stderr: {stderr}");
}

#[test]
fn test_config_fail_missing_rpc_address() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		remove_config_line(&default_test_config(params), "rpc_address =")
	});
	assert!(stderr.contains("Missing `bitcoind_rpc_address`"), "Unexpected stderr: {stderr}");
}

#[test]
fn test_config_fail_missing_rpc_user() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		remove_config_line(&default_test_config(params), "rpc_user =")
	});
	assert!(stderr.contains("Missing `bitcoind_rpc_user`"), "Unexpected stderr: {stderr}");
}

#[test]
fn test_config_fail_missing_rpc_password() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		remove_config_line(&default_test_config(params), "rpc_password =")
	});
	assert!(stderr.contains("Missing `bitcoind_rpc_password`"), "Unexpected stderr: {stderr}");
}

#[test]
fn test_config_fail_multiple_chain_sources() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		let mut config = default_test_config(params);
		config.push_str("\n[esplora]\nserver_url = \"https://mempool.space/api\"\n");
		config
	});
	assert!(stderr.contains("Must set a single chain source"), "Unexpected stderr: {stderr}");
}

#[test]
fn test_config_fail_invalid_rest_service_address() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		replace_config_line(
			&default_test_config(params),
			"rest_service_address =",
			"rest_service_address = \"not-a-valid-address\"",
		)
	});
	assert!(stderr.contains("Invalid configuration"), "Unexpected stderr: {stderr}");
}

#[test]
fn test_config_fail_invalid_listening_address() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		replace_config_line(
			&default_test_config(params),
			"listening_addresses =",
			"listening_addresses = [\"definitely not an address\"]",
		)
	});
	assert!(stderr.contains("Invalid listening addresses"), "Unexpected stderr: {stderr}");
}

#[test]
fn test_config_fail_alias_too_long() {
	let bitcoind = TestBitcoind::new();
	let long_alias = "a".repeat(33);
	let stderr = start_expect_failure(&bitcoind, |params| {
		replace_config_line(
			&default_test_config(params),
			"alias =",
			&format!("alias = \"{}\"", long_alias),
		)
	});
	assert!(stderr.contains("alias") && stderr.contains("32 bytes"), "Unexpected stderr: {stderr}");
}

#[test]
fn test_config_fail_invalid_log_level() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		let mut config = default_test_config(params);
		config.push_str("\n[log]\nlevel = \"NotALevel\"\n");
		config
	});
	assert!(
		stderr.contains("Invalid log level") || stderr.contains("Invalid configuration"),
		"Unexpected stderr: {stderr}"
	);
}

#[test]
fn test_config_fail_missing_rabbitmq() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		remove_config_section(&default_test_config(params), "[rabbitmq]")
	});
	assert!(stderr.contains("rabbitmq"), "Unexpected stderr: {stderr}");
}

#[test]
fn test_config_fail_missing_lsps2() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		remove_config_section(&default_test_config(params), "[liquidity.lsps2_service]")
	});
	assert!(
		stderr.contains("lsps2") || stderr.contains("liquidity"),
		"Unexpected stderr: {stderr}"
	);
}

#[test]
fn test_config_fail_invalid_toml() {
	let bitcoind = TestBitcoind::new();
	let stderr =
		start_expect_failure(&bitcoind, |_params| "this is not valid [[ toml {{{{".to_string());
	assert!(
		stderr.contains("invalid TOML") || stderr.contains("Invalid configuration"),
		"Unexpected stderr: {stderr}"
	);
}

#[test]
fn test_config_fail_alias_without_listening_addresses() {
	let bitcoind = TestBitcoind::new();
	let stderr = start_expect_failure(&bitcoind, |params| {
		remove_config_line(&default_test_config(params), "listening_addresses =")
	});
	assert!(
		stderr.contains("Listening addresses") || stderr.contains("listening addresses"),
		"Unexpected stderr: {stderr}"
	);
}

#[tokio::test]
async fn test_config_chain_source_bitcoind_localhost() {
	let bitcoind = TestBitcoind::new();
	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		// Use "localhost:port" instead of "127.0.0.1:port" to test hostname RPC support
		let rpc_address = params.rpc_address.replace("127.0.0.1", "localhost");
		test_config_with_chain_source(
			params,
			&format!(
				"[bitcoind]\nrpc_address = \"{}\"\nrpc_user = \"{}\"\nrpc_password = \"{}\"",
				rpc_address, params.rpc_user, params.rpc_password
			),
		)
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_chain_source_esplora() {
	let bitcoind = TestBitcoind::new();
	let electrs = TestElectrs::new(&bitcoind);
	let esplora_url = electrs.esplora_url();

	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		test_config_with_chain_source(
			params,
			&format!("[esplora]\nserver_url = \"{}\"", esplora_url),
		)
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}

#[tokio::test]
async fn test_config_chain_source_electrum() {
	let bitcoind = TestBitcoind::new();
	let electrs = TestElectrs::new(&bitcoind);
	let electrum_url = electrs.electrum_url();

	let server = LdkServerHandle::start_with_config(&bitcoind, |params| {
		test_config_with_chain_source(
			params,
			&format!("[electrum]\nserver_url = \"{}\"", electrum_url),
		)
	})
	.await;
	let info = server.client().get_node_info(GetNodeInfoRequest {}).await.unwrap();
	assert!(info.current_best_block.is_some());
}
