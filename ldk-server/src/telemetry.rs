use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use ldk_node::{BalanceDetails, Node};
use metrics::{describe_counter, describe_gauge, gauge};

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

pub fn setup_prometheus() -> PrometheusHandle {
	let prometheus_builder = PrometheusBuilder::new();
	let handler =
		prometheus_builder.install_recorder().expect("failed to install Prometheus recorder");

	describe_counter!(
		"channel_pending",
		"A channel has been created and is pending confirmation on-chain."
	);
	describe_counter!("channel_ready", "A channel is ready to be used.");
	describe_counter!("payment_received", "A payment has been received.");
	describe_counter!("payment_successful", "A sent payment was successful.");
	describe_counter!("payment_failed", "A sent payment has failed.");
	describe_counter!(
		"payment_claimable",
		"A payment for a previously-registered payment hash has been received."
	);
	describe_counter!("payment_forwarded", "A sent payment has failed.");

	describe_gauge!("node_total_onchain_balance_sats", "The total balance of our on-chain wallet.");
	describe_gauge!(
		"node_spendable_onchain_balance_sats",
		"The currently spendable balance of our on-chain wallet."
	);
	describe_gauge!(
		"node_total_anchor_channels_reserve_sats",
		"The total anchor channel reserve balance."
	);
	describe_gauge!(
		"node_total_lightning_balance_sats",
		"The total balance that we would be able to claim across all our Lightning channels."
	);
	describe_gauge!(
		"node_lightning_balances",
		"A detailed list of all known Lightning balances that would be claimable on channel closure."
	);
	describe_gauge!(
		"node_pending_balances_from_channel_closures",
		"A detailed list of balances currently being swept from the Lightning to the on-chain wallet."
	);

	// TODO (arturgontijo): Add all labels here. Fix descriptions.

	handler
}

pub async fn collect_node_metrics(node: Arc<Node>) -> io::Result<()> {
	println!("collect_node_metrics...");
	let BalanceDetails {
		total_onchain_balance_sats,
		spendable_onchain_balance_sats,
		total_anchor_channels_reserve_sats,
		total_lightning_balance_sats,
		// TODO (arturgontijo):
		// lightning_balances,
		// pending_balances_from_channel_closures,
		..
	} = node.list_balances();
	set_gauge("node_total_onchain_balance_sats".to_string(), total_onchain_balance_sats as f64);
	set_gauge(
		"node_spendable_onchain_balance_sats".to_string(),
		spendable_onchain_balance_sats as f64,
	);
	set_gauge(
		"node_total_anchor_channels_reserve_sats".to_string(),
		total_anchor_channels_reserve_sats as f64,
	);
	set_gauge("node_total_lightning_balance_sats".to_string(), total_lightning_balance_sats as f64);
	// TODO (arturgontijo):
	// set_gauge("node_lightning_balances".to_string(), lightning_balances as f64);
	// set_gauge("node_pending_balances_from_channel_closures".to_string(), pending_balances_from_channel_closures as f64);

	// TODO (arturgontijo): Get sleep delay from config file.
	sleep(Duration::from_millis(10_000)).await;

	Ok(())
}

fn set_gauge(label: String, value: f64) {
	let gauge = gauge!(label);
	gauge.set(value);
}
