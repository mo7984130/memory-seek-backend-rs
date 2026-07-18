use std::net::ToSocketAddrs;

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

impl Default for Config {
    fn default() -> Self {
        Self {
            host: default_metrics_host(),
            port: default_metrics_port(),
        }
    }
}
fn default_metrics_host() -> String {
    "0.0.0.0".to_string()
}
const fn default_metrics_port() -> u16 {
    9090
}

/// 初始化 Prometheus metrics exporter
pub fn init(cfg: &Config) {
    info!(
        "Prometheus metrics exporter will listen on {}:{}",
        cfg.host, cfg.port
    );

    let addr = format!("{}:{}", cfg.host, cfg.port)
        .to_socket_addrs()
        .expect("Failed to parse metrics address")
        .next()
        .expect("No socket address found");

    PrometheusBuilder::new()
        .with_http_listener(addr)
        .install()
        .expect("Failed to start Prometheus metrics exporter");

    info!("Prometheus metrics exporter listening on {}", addr);
}
