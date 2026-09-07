mod database;
mod redis;
mod system;

use std::mem::take;

use axum::{Router, middleware::from_fn, routing::get};
use common::{Pool, Result, register_async, time::Duration};
use metrics_exporter_prometheus::PrometheusBuilder;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use sysinfo::System;
use tokio::select;
use tracing::{debug, info};

use crate::{
    config::AppConfig, middlewares::metrics::metrics_middleware, setup::AppSetup,
    util::MissDepError,
};

/// 所有 histogram 的统一分桶（单位：秒）。
/// 覆盖 HTTP 请求延迟与业务操作耗时，末尾保留大桶容纳慢操作。
const DURATION_BUCKETS: &[f64] = &[0.01, 0.05, 0.1, 0.3, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0];

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

#[register_async(
    slice = crate::setup::bases::APP_BASES_LAST,
    ty = crate::setup::InitFn
)]
pub async fn init(config: &AppConfig, setup: &mut AppSetup) -> Result<()> {
    debug!("正在初始化 Prometheus 指标");
    let config = &config.metrics;

    let cancel_token = setup.cancel_token.child_token();
    let upkeep_interval = Duration::from_secs(config.interval_seconds);

    let handle = PrometheusBuilder::new()
        .set_buckets(DURATION_BUCKETS)
        .expect("设置 Prometheus 指标分桶失败")
        .install_recorder()
        .expect("安装 Prometheus 指标记录器失败");

    let db = setup
        .registry
        .get::<DatabaseConnection>()
        .miss_dep("metrics", "DatabaseConnection")?
        .clone();
    let redis = setup
        .registry
        .get::<Pool>()
        .miss_dep("metrics", "RedisPool")?
        .clone();

    // 后台执行 recorder upkeep
    let upkeep_handle = handle.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(upkeep_interval);
        let mut sys = System::new_all();

        loop {
            select! {
                _ = cancel_token.cancelled() => {
                    tracing::info!("指标采集任务收到取消信号，正在退出");
                    break;
                }
                _ = tick.tick() => {
                    upkeep_handle.run_upkeep();
                    sys.refresh_all();
                    system::collect_system_metrics(&mut sys);
                    database::collect_db_metrics(&db);
                    redis::collect_redis_metrics(&redis);
                }
            }
        }
    });

    // 暴露 Prometheus /metrics 接口
    let metrics_handle = handle.clone();

    setup.router.add_public(Router::new().route(
        "/metrics",
        get(move || async move { metrics_handle.render() }),
    ));
    setup.router.public = take(&mut setup.router.public).layer(from_fn(metrics_middleware));
    setup.router.protected = take(&mut setup.router.protected).layer(from_fn(metrics_middleware));

    info!("Prometheus 指标初始化完成");

    Ok(())
}
