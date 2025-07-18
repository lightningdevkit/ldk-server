use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

pub fn setup_prometheus() -> PrometheusHandle {
	let prometheus_builder = PrometheusBuilder::new();
	let handler =
		prometheus_builder.install_recorder().expect("failed to install Prometheus recorder");
	handler
}
