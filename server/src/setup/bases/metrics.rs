use common::time::Duration;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use serde::Deserialize;
use tracing::info;

/// 所有 histogram 的统一分桶（单位：秒）。
/// 覆盖 HTTP 请求延迟与业务操作耗时，末尾保留大桶容纳慢操作。
const DURATION_BUCKETS: &[f64] = &[0.01, 0.05, 0.1, 0.3, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0];

/// 指标 upkeep 周期，与原 http-listener 内部默认一致（避免 idle 指标无限增长）。
const UPKEEP_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_interval_seconds")]
    pub interval_seconds: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval_seconds: default_interval_seconds(),
        }
    }
}
const fn default_interval_seconds() -> u64 {
    5
}

/// 初始化 Prometheus recorder（不再单独监听端口，由主服务暴露 /metrics）。
///
/// 返回 `PrometheusHandle` 用于渲染 Prometheus 文本格式；同时在后台周期性执行
/// upkeep，回收 idle 指标，避免内存无限增长。
pub fn init(cfg: &Config) -> PrometheusHandle {
    info!(
        "Prometheus metrics recorder installed, upkeep interval: {:?}, collect interval: {}s",
        UPKEEP_INTERVAL, cfg.interval_seconds
    );

    let handle = PrometheusBuilder::new()
        .set_buckets(DURATION_BUCKETS)
        .expect("Failed to set metric buckets")
        .install_recorder()
        .expect("Failed to install Prometheus recorder");

    let upkeep_handle = handle.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(UPKEEP_INTERVAL);
        loop {
            tick.tick().await;
            upkeep_handle.run_upkeep();
        }
    });

    info!("Prometheus metrics recorder ready");
    handle
}
