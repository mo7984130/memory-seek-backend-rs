use metrics_exporter_prometheus::PrometheusBuilder;
use tracing::info;

/// 初始化 Prometheus metrics exporter
pub fn init(host: &str, port: u16) {
    PrometheusBuilder::new()
        .with_http_listener((host.parse::<std::net::IpAddr>().unwrap(), port))
        .install()
        .expect("Failed to start Prometheus metrics exporter");
    info!("Prometheus metrics exporter listening on {}:{}", host, port);
}
