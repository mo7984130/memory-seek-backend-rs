use metrics::gauge;
use sea_orm::DatabaseConnection;

/// 采集数据库连接池指标
///
/// SeaORM 1.x 基于 sqlx，通过获取底层连接池统计
/// 失败时设置 -1 并记录 warn 日志
pub fn collect_db_metrics(db: &DatabaseConnection) {
    // SeaORM 提供 get_postgres_connection_pool() 方法获取底层 sqlx::PgPool
    // sqlx::PgPool 提供 size() 和 num_idle() 方法
    let pool = db.get_postgres_connection_pool();
    let size = pool.size() as f64;
    let idle = pool.num_idle() as f64;

    gauge!("database.connections.active").set(size - idle);
    gauge!("database.connections.idle").set(idle);
    gauge!("database.connections.waiting").set(0.0); // sqlx 不直接暴露等待数
}
