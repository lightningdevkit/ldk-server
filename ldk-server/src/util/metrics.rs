// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::Duration;

use ldk_node::payment::PaymentStatus;
use ldk_node::Node;

pub const BUILD_METRICS_INTERVAL: Duration = Duration::from_secs(60);

pub struct Metrics {
	pub total_peers_count: AtomicI64,
	pub total_payments_count: AtomicI64,
	pub total_successful_payments_count: AtomicI64,
	pub total_failed_payments_count: AtomicI64,
	pub total_channels_count: AtomicI64,
	pub total_public_channels_count: AtomicI64,
	pub total_private_channels_count: AtomicI64,
	pub total_onchain_balance_sats: AtomicU64,
	pub spendable_onchain_balance_sats: AtomicU64,
	pub total_anchor_channels_reserve_sats: AtomicU64,
	pub total_lightning_balance_sats: AtomicU64,
}

impl Metrics {
	pub fn new() -> Self {
		Self {
			total_peers_count: AtomicI64::new(0),
			total_payments_count: AtomicI64::new(0),
			total_successful_payments_count: AtomicI64::new(0),
			total_failed_payments_count: AtomicI64::new(0),
			total_channels_count: AtomicI64::new(0),
			total_public_channels_count: AtomicI64::new(0),
			total_private_channels_count: AtomicI64::new(0),
			total_onchain_balance_sats: AtomicU64::new(0),
			spendable_onchain_balance_sats: AtomicU64::new(0),
			total_anchor_channels_reserve_sats: AtomicU64::new(0),
			total_lightning_balance_sats: AtomicU64::new(0),
		}
	}

	fn update_peer_count(&self, node: &Node) {
		let total_peers_count = node.list_peers().len() as i64;
		self.total_peers_count.store(total_peers_count, Ordering::Relaxed);
	}

	fn update_total_payments_count(&self, node: &Node) {
		let total_payments_count = node.list_payments().len() as i64;
		self.total_payments_count.store(total_payments_count, Ordering::Relaxed);
	}

	pub fn update_total_successful_payments_count(&self, node: &Node) {
		let successful_payments_count = node
			.list_payments()
			.iter()
			.filter(|payment_details| payment_details.status == PaymentStatus::Succeeded)
			.count() as i64;
		self.total_successful_payments_count.store(successful_payments_count, Ordering::Relaxed);
	}

	pub fn update_total_failed_payments_count(&self, node: &Node) {
		let failed_payments_count = node
			.list_payments()
			.iter()
			.filter(|payment_details| payment_details.status == PaymentStatus::Failed)
			.count() as i64;
		self.total_failed_payments_count.store(failed_payments_count, Ordering::Relaxed);
	}

	fn update_total_channels_count(&self, node: &Node) {
		let total_channels_count = node.list_channels().len() as i64;
		self.total_channels_count.store(total_channels_count, Ordering::Relaxed);
	}

	fn update_total_public_channels_count(&self, node: &Node) {
		let total_public_channels_count = node
			.list_channels()
			.iter()
			.filter(|channel_details| channel_details.is_announced)
			.count() as i64;
		self.total_public_channels_count.store(total_public_channels_count, Ordering::Relaxed);
	}

	fn update_total_private_channels_count(&self, node: &Node) {
		let total_private_channels_count = node
			.list_channels()
			.iter()
			.filter(|channel_details| !channel_details.is_announced)
			.count() as i64;
		self.total_private_channels_count.store(total_private_channels_count, Ordering::Relaxed);
	}

	fn update_all_balances(&self, node: &Node) {
		let all_balances = node.list_balances();
		self.total_onchain_balance_sats
			.store(all_balances.total_onchain_balance_sats, Ordering::Relaxed);

		self.spendable_onchain_balance_sats
			.store(all_balances.spendable_onchain_balance_sats, Ordering::Relaxed);

		self.total_anchor_channels_reserve_sats
			.store(all_balances.total_anchor_channels_reserve_sats, Ordering::Relaxed);

		self.total_lightning_balance_sats
			.store(all_balances.total_lightning_balance_sats, Ordering::Relaxed);
	}

	pub fn update_all_pollable_metrics(&self, node: &Node) {
		self.update_peer_count(node);
		self.update_total_payments_count(node);
		self.update_total_successful_payments_count(node);
		self.update_total_failed_payments_count(node);
		self.update_total_channels_count(node);
		self.update_total_public_channels_count(node);
		self.update_total_private_channels_count(node);
		self.update_all_balances(node);
	}

	pub fn gather_metrics(&self) -> String {
		let mut buffer = String::new();

		fn format_metric(
			buffer: &mut String, name: &str, help: &str, metric_type: &str,
			value: impl std::fmt::Display,
		) {
			use std::fmt::Write;
			let _ = writeln!(buffer, "# HELP {} {}", name, help);
			let _ = writeln!(buffer, "# TYPE {} {}", name, metric_type);
			let _ = writeln!(buffer, "{} {}", name, value);
		}

		format_metric(
			&mut buffer,
			"ldk_server_total_peers_count",
			"Total number of peers",
			"gauge",
			self.total_peers_count.load(Ordering::Relaxed),
		);

		format_metric(
			&mut buffer,
			"ldk_server_total_payments_count",
			"Total number of payments",
			"counter",
			self.total_payments_count.load(Ordering::Relaxed),
		);

		format_metric(
			&mut buffer,
			"ldk_server_total_successful_payments_count",
			"Total number of successful payments",
			"counter",
			self.total_successful_payments_count.load(Ordering::Relaxed),
		);

		format_metric(
			&mut buffer,
			"ldk_server_total_failed_payments_count",
			"Total number of failed payments",
			"counter",
			self.total_failed_payments_count.load(Ordering::Relaxed),
		);

		format_metric(
			&mut buffer,
			"ldk_server_total_channels_count",
			"Total number of channels",
			"counter",
			self.total_channels_count.load(Ordering::Relaxed),
		);

		format_metric(
			&mut buffer,
			"ldk_server_total_public_channels_count",
			"Total number of public channels",
			"counter",
			self.total_public_channels_count.load(Ordering::Relaxed),
		);

		format_metric(
			&mut buffer,
			"ldk_server_total_private_channels_count",
			"Total number of private channels",
			"counter",
			self.total_private_channels_count.load(Ordering::Relaxed),
		);

		format_metric(
			&mut buffer,
			"ldk_server_total_onchain_balance_sats",
			"Total onchain balance in sats",
			"gauge",
			self.total_onchain_balance_sats.load(Ordering::Relaxed),
		);

		format_metric(
			&mut buffer,
			"ldk_server_spendable_onchain_balance_sats",
			"Spendable onchain balance in sats",
			"gauge",
			self.spendable_onchain_balance_sats.load(Ordering::Relaxed),
		);

		format_metric(
			&mut buffer,
			"ldk_server_total_anchor_channels_reserve_sats",
			"Total anchor channels reserve in sats",
			"gauge",
			self.total_anchor_channels_reserve_sats.load(Ordering::Relaxed),
		);

		format_metric(
			&mut buffer,
			"ldk_server_total_lightning_balance_sats",
			"Total lightning balance in sats",
			"gauge",
			self.total_lightning_balance_sats.load(Ordering::Relaxed),
		);

		buffer
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_initial_metrics_values() {
		let metrics = Metrics::new();
		let result = metrics.gather_metrics();

		// Check that all metrics are present and empty
		assert!(result.contains("ldk_server_total_peers_count 0"));
		assert!(result.contains("ldk_server_total_payments_count 0"));
		assert!(result.contains("ldk_server_total_successful_payments_count 0"));
		assert!(result.contains("ldk_server_total_failed_payments_count 0"));
		assert!(result.contains("ldk_server_total_channels_count 0"));
		assert!(result.contains("ldk_server_total_public_channels_count 0"));
		assert!(result.contains("ldk_server_total_private_channels_count 0"));
		assert!(result.contains("ldk_server_total_onchain_balance_sats 0"));
		assert!(result.contains("ldk_server_spendable_onchain_balance_sats 0"));
		assert!(result.contains("ldk_server_total_anchor_channels_reserve_sats 0"));
		assert!(result.contains("ldk_server_total_lightning_balance_sats 0"));
	}

	#[test]
	fn test_metrics_update_and_gather() {
		let metrics = Metrics::new();

		// Manually update metrics to simulate node activity
		metrics.total_peers_count.store(5, Ordering::Relaxed);
		metrics.total_payments_count.store(10, Ordering::Relaxed);
		metrics.total_successful_payments_count.store(8, Ordering::Relaxed);
		metrics.total_failed_payments_count.store(2, Ordering::Relaxed);
		metrics.total_channels_count.store(3, Ordering::Relaxed);
		metrics.total_public_channels_count.store(1, Ordering::Relaxed);
		metrics.total_private_channels_count.store(2, Ordering::Relaxed);
		metrics.total_onchain_balance_sats.store(100_000, Ordering::Relaxed);
		metrics.spendable_onchain_balance_sats.store(50_000, Ordering::Relaxed);
		metrics.total_anchor_channels_reserve_sats.store(1_000, Ordering::Relaxed);
		metrics.total_lightning_balance_sats.store(250_000, Ordering::Relaxed);

		let result = metrics.gather_metrics();

		// Check that output contains updated values and correct Prometheus format
		assert!(result.contains("# HELP ldk_server_total_peers_count Total number of peers"));
		assert!(result.contains("# TYPE ldk_server_total_peers_count gauge"));
		assert!(result.contains("ldk_server_total_peers_count 5"));

		assert!(result.contains("ldk_server_total_payments_count 10"));
		assert!(result.contains("ldk_server_total_successful_payments_count 8"));
		assert!(result.contains("ldk_server_total_failed_payments_count 2"));
		assert!(result.contains("ldk_server_total_channels_count 3"));
		assert!(result.contains("ldk_server_total_public_channels_count 1"));
		assert!(result.contains("ldk_server_total_private_channels_count 2"));
		assert!(result.contains("ldk_server_total_onchain_balance_sats 100000"));
		assert!(result.contains("ldk_server_spendable_onchain_balance_sats 50000"));
		assert!(result.contains("ldk_server_total_anchor_channels_reserve_sats 1000"));
		assert!(result.contains("ldk_server_total_lightning_balance_sats 250000"));
	}
}
