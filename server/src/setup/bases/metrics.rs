use metrics_exporter_prometheus::PrometheusBuilder;
use tracing::info;

/// 初始化 Prometheus metrics exporter
/// 监听 0.0.0.0:9090，提供 /metrics endpoint
pub fn init() -> anyhow::Result<()> {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9090))
        .install()?;
    info!("Prometheus metrics exporter listening on 0.0.0.0:9090");
    Ok(())
}
