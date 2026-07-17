use metrics_exporter_prometheus::PrometheusBuilder;
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_metrics_host")]
    pub host: String,
    #[serde(default = "default_metrics_port")]
    pub port: u16,
}
fn default_metrics_host() -> String {
    "0.0.0.0".to_string()
}
const fn default_metrics_port() -> u16 {
    9090
}

/// 初始化 Prometheus metrics exporter
pub fn init(cfg: &Config) {
    let addr = cfg.host.parse::<std::net::IpAddr>().unwrap();
    PrometheusBuilder::new()
        .with_http_listener((addr, cfg.port))
        .install()
        .expect("Failed to start Prometheus metrics exporter");
    info!(
        "Prometheus metrics exporter listening on {}:{}",
        cfg.host, cfg.port
    );
}
