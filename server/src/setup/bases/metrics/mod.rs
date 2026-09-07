mod database;
mod redis;
mod system;

use std::mem::take;
use std::sync::Arc;

use axum::{Router, middleware::from_fn, routing::get};
use common::tokio::TaskManager;
use common::{Pool, Result, register_async, time::Duration};
use metrics_exporter_prometheus::PrometheusBuilder;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use sysinfo::System;
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

    let task_manager = setup
        .registry
        .get::<TaskManager>()
        .miss_dep("metrics", "TaskManager")?
        .clone();
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

    // 常驻后台任务：指标 recorder upkeep 与周期性采集，由 TaskManager 托管。
    // `System` 采样器跨周期复用，用互斥锁共享；等待阶段可被取消，执行阶段为瞬时同步采集。
    let sys = Arc::new(tokio::sync::Mutex::new(System::new_all()));
    let upkeep_handle = handle.clone();
    task_manager.spawn_interval("metrics_upkeep", upkeep_interval, move || {
        let sys = sys.clone();
        let upkeep_handle = upkeep_handle.clone();
        let db = db.clone();
        let redis = redis.clone();
        async move {
            upkeep_handle.run_upkeep();
            let mut sys = sys.lock().await;
            system::collect_system_metrics(&mut sys);
            database::collect_db_metrics(&db);
            redis::collect_redis_metrics(&redis);
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
