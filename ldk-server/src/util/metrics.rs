// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use ldk_node::Node;

pub const BUILD_METRICS_INTERVAL: Duration = Duration::from_secs(60);

/// This represents a [`Metrics`] type that can go up and down in value.
pub struct IntGauge {
	inner: AtomicI64,
}

impl IntGauge {
	pub fn new() -> Self {
		Self { inner: AtomicI64::new(0) }
	}

	pub fn set(&self, value: i64) {
		self.inner.store(value, Ordering::Relaxed);
	}

	pub fn get(&self) -> i64 {
		self.inner.load(Ordering::Relaxed)
	}
}

/// Represents the [`Metrics`] output values and type.
pub struct MetricsOutput {
	name: String,
	help_text: String,
	metric_type: String,
	value: String,
}

impl MetricsOutput {
	pub fn new(name: &str, help_text: &str, metric_type: &str, value: &str) -> Self {
		Self {
			name: name.to_string(),
			help_text: help_text.to_string(),
			metric_type: metric_type.to_string(),
			value: value.to_string(),
		}
	}
}

pub struct Metrics {
	pub service_health_score: IntGauge,
}

impl Metrics {
	pub fn new() -> Self {
		Self { service_health_score: IntGauge::new() }
	}

	pub fn update_service_health_score(&self, node: &Node) {
		let score = self.calculate_ldk_server_health_score(node);
		self.service_health_score.set(score);
	}

	/// The health score computation is pretty basic for now and simply
	/// calculated based on the impacted events on the components of the
	/// `Node`. The events severity and weightage value are as follows:
	///
	/// - Critical: 0 (Total failure)
	/// - Major: 35%
	/// - Minor: 25%
	///
	/// Using the assigned score above, the health score of the `Node` is
	/// computed as:
	///
	/// Health score = Maximum health score - Sum(Event severity score)
	///
	/// Where:
	///
	/// - Maximum health score = 100
	///
	/// If the `Node` is not running/online, i.e `is_running` is false,
	/// the severity is critical with a weightage value of -100%.
	///
	/// If the `Node` is running but isn't connected to any peer yet,
	/// the severity is major with a weightage value of -35%.
	///
	/// If the `Node` is running but the Lightning Wallet hasn't been synced
	/// yet, the severity is minor with a weightage value of -25%.
	pub fn calculate_ldk_server_health_score(&self, node: &Node) -> i64 {
		Self::compute_health_score(
			node.status().is_running,
			!node.list_peers().is_empty(),
			node.status().latest_lightning_wallet_sync_timestamp.is_some(),
		)
	}

	pub fn format_metrics_output(&self, buffer: &mut String, options: &MetricsOutput) {
		buffer.push_str(&format!("# HELP {} {}\n", options.name, options.help_text));
		buffer.push_str(&format!("# TYPE {} {}\n", options.name, options.metric_type));
		buffer.push_str(&format!("{} {}\n", options.name, options.value));
	}

	pub fn gather_metrics(&self) -> String {
		let mut buffer = String::new();
		let options = &MetricsOutput::new(
			"ldk_server_health_score",
			"Current health score (0-100)",
			"gauge",
			&self.service_health_score.get().to_string(),
		);

		self.format_metrics_output(&mut buffer, options);

		buffer
	}

	fn compute_health_score(is_running: bool, has_peers: bool, is_wallet_synced: bool) -> i64 {
		if !is_running {
			return 0;
		}

		let mut health_score = 100;

		if !has_peers {
			health_score -= 35;
		}

		if !is_wallet_synced {
			health_score -= 25;
		}

		health_score
	}
}

#[cfg(test)]
mod tests {

	use super::*;

	#[test]
	fn test_compute_health_score() {
		// Node is not running
		assert_eq!(Metrics::compute_health_score(false, true, true), 0);
		assert_eq!(Metrics::compute_health_score(false, false, false), 0);

		// Node is running, connected to a peer and wallet is synced
		assert_eq!(Metrics::compute_health_score(true, true, true), 100);

		// Node is running, not connected to a peer but wallet is synced
		assert_eq!(Metrics::compute_health_score(true, false, true), 65);

		// Node is running, connected to a peer but wallet is not synced
		assert_eq!(Metrics::compute_health_score(true, true, false), 75);

		// Node is running, not connected to a peer and wallet is not synced
		assert_eq!(Metrics::compute_health_score(true, false, false), 40);
	}

	#[test]
	fn test_gather_metrics_format() {
		let metrics = Metrics::new();

		let result = metrics.gather_metrics();
		assert!(result.contains("ldk_server_health_score"));
	}
}
