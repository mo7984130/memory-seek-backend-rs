#[cfg(feature = "metrics")]
mod database;
#[cfg(feature = "metrics")]
mod redis;
#[cfg(feature = "metrics")]
mod system;

#[cfg(feature = "metrics")]
use std::time::Duration;
#[cfg(feature = "metrics")]
use sysinfo::System;
#[cfg(feature = "metrics")]
use tokio::time::interval;
#[cfg(feature = "metrics")]
use tokio_util::sync::CancellationToken;

/// 启动后台指标采集任务
///
/// 每 5 秒采集一次系统指标、数据库连接池指标、Redis 连接池指标。
/// 当 `cancel_token` 被取消时，任务会优雅退出。
#[cfg(feature = "metrics")]
pub fn start_collector(
    db: sea_orm::DatabaseConnection,
    redis_pool: deadpool_redis::Pool,
    cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let mut sys = System::new_all();
        let mut tick = interval(Duration::from_secs(5));

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
