mod database;
mod redis;
mod system;

use std::time::Duration;
use sysinfo::System;
use tokio::time::interval;

/// 启动后台指标采集任务
///
/// 每 15 秒采集一次系统指标、数据库连接池指标、Redis 连接池指标
pub fn start_collector(db: sea_orm::DatabaseConnection, redis_pool: deadpool_redis::Pool) {
    tokio::spawn(async move {
        let mut sys = System::new_all();
        let mut tick = interval(Duration::from_secs(15));

        loop {
            tick.tick().await;
            system::collect_system_metrics(&mut sys);
            database::collect_db_metrics(&db);
            redis::collect_redis_metrics(&redis_pool);
        }
    });
}
