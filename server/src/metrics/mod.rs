#[cfg(feature = "metrics")]
mod database;
#[cfg(feature = "metrics")]
mod redis;
#[cfg(feature = "metrics")]
mod system;

#[cfg(feature = "metrics")]
use std::sync::Arc;
#[cfg(feature = "metrics")]
use std::time::Duration;
#[cfg(feature = "metrics")]
use sysinfo::System;
#[cfg(feature = "metrics")]
use tokio_util::sync::CancellationToken;

/// 渲染 Prometheus 文本格式快照，作为主服务 `GET /metrics` 的响应体。
#[cfg(feature = "metrics")]
pub async fn render_metrics(
    axum::extract::State(state): axum::extract::State<Arc<crate::state::AppState>>,
) -> String {
    state.metrics_handle.render()
}

/// 启动后台指标采集任务
///
/// 按 `interval` 周期采集系统指标、数据库连接池指标、Redis 连接池指标。
/// 启动时先写入一次 `server.build_info` 版本指标。
/// 当 `cancel_token` 被取消时，任务会优雅退出。
#[cfg(feature = "metrics")]
pub fn start_collector(
    db: sea_orm::DatabaseConnection,
    redis_pool: deadpool_redis::Pool,
    interval: Duration,
    cancel_token: CancellationToken,
) {
    metrics::gauge!(
        "server.build_info",
        "version" => env!("CARGO_PKG_VERSION").to_string(),
        "commit" => option_env!("GIT_COMMIT").unwrap_or("unknown").to_string()
    )
    .set(1.0);

    tokio::spawn(async move {
        let mut sys = System::new_all();
        let mut tick = tokio::time::interval(interval);

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    tracing::info!("指标采集任务收到取消信号，正在退出");
                    break;
                }
                _ = tick.tick() => {
                    sys.refresh_all();
                    system::collect_system_metrics(&mut sys);
                    database::collect_db_metrics(&db);
                    redis::collect_redis_metrics(&redis_pool);
                }
            }
        }
    });
}
