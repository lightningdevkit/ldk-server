// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::time::Duration;

use lazy_static::lazy_static;
use ldk_node::Node;
use prometheus::{
	default_registry, gather, register_int_gauge_with_registry, Encoder, IntGauge, Opts, Registry,
	TextEncoder,
};

use crate::api::error::LdkServerError;

pub const BUILD_METRICS_INTERVAL: Duration = Duration::from_secs(60);

lazy_static! {
	pub static ref METRICS: Metrics = Metrics::new(default_registry());
}

pub struct Metrics {
	pub service_health_score: IntGauge,
}

impl Metrics {
	pub fn new(registry: &Registry) -> Self {
		Self {
			service_health_score: register_int_gauge_with_registry!(
				Opts::new("ldk_health_score", "Current health score (0-100)"),
				registry
			)
			.expect("Failed to register metric"),
		}
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

	pub fn gather_metrics(&self) -> Result<String, LdkServerError> {
		let mut buffer = Vec::new();
		let encoder = TextEncoder::new();

		let all_metrics = gather();
		encoder.encode(&all_metrics, &mut buffer)?;
		Ok(String::from_utf8(buffer)?)
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
		let result = METRICS.gather_metrics();
		assert!(result.is_ok());
		let output = result.unwrap();
		assert!(output.contains("ldk_health_score"));
	}
}
