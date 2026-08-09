use std::net::ToSocketAddrs;

use metrics_exporter_prometheus::PrometheusBuilder;
use serde::Deserialize;
use tracing::info;

/// 所有 histogram 的统一分桶（单位：秒）。
/// 覆盖 HTTP 请求延迟与业务操作耗时，末尾保留大桶容纳慢操作。
const DURATION_BUCKETS: &[f64] = &[0.01, 0.05, 0.1, 0.3, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0];

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_metrics_host")]
    pub host: String,
    #[serde(default = "default_metrics_port")]
    pub port: u16,
    #[serde(default = "default_interval_seconds")]
    pub interval_seconds: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: default_metrics_host(),
            port: default_metrics_port(),
            interval_seconds: default_interval_seconds(),
        }
    }
}
fn default_metrics_host() -> String {
    "0.0.0.0".to_string()
}
const fn default_metrics_port() -> u16 {
    9090
}
const fn default_interval_seconds() -> u64 {
    5
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
        .set_buckets(DURATION_BUCKETS)
        .expect("Failed to set metric buckets")
        .with_http_listener(addr)
        .install()
        .expect("Failed to start Prometheus metrics exporter");

    info!("Prometheus metrics exporter listening on {}", addr);
}
