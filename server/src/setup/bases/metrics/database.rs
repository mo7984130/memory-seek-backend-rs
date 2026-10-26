use metrics::gauge;
use sea_orm::DatabaseConnection;

/// 采集数据库连接池指标，获取失败时记 -1 并打 warn 日志
pub fn collect_db_metrics(db: &DatabaseConnection) {
    // SeaORM 提供 get_postgres_connection_pool() 方法获取底层 sqlx::PgPool
    // sqlx::PgPool 提供 size() 和 num_idle() 方法
    let pool = db.get_postgres_connection_pool();
    let size = pool.size() as f64;
    let idle = pool.num_idle() as f64;

    gauge!("database.connections.active").set(size - idle);
    gauge!("database.connections.idle").set(idle);
    gauge!("database.connections.max").set(pool.options().get_max_connections() as f64);
}
