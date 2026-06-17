use metrics::gauge;
use sea_orm::DatabaseConnection;
use tracing::warn;

/// 采集数据库连接池指标
///
/// SeaORM 1.x 基于 sqlx，通过 get_pool_status() 获取连接池统计
/// 失败时设置 -1 并记录 warn 日志
pub fn collect_db_metrics(db: &DatabaseConnection) {
    // 注意：SeaORM 1.1.19 的 get_pool_status() 需要验证可用性
    // 如果 API 不可用，将使用 -1 表示指标不可用
    match db.get_pool_status() {
        Ok(status) => {
            gauge!("database.connections.active").set(status.active as f64);
            gauge!("database.connections.idle").set(status.idle as f64);
            gauge!("database.connections.waiting").set(status.waiting as f64);
        }
        Err(e) => {
            warn!("获取数据库连接池状态失败: {}", e);
            gauge!("database.connections.active").set(-1.0);
            gauge!("database.connections.idle").set(-1.0);
            gauge!("database.connections.waiting").set(-1.0);
        }
    }
}
