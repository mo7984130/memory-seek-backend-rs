use std::time::Duration;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use serde::Deserialize;
use crate::config::AppConfig;

#[derive(Clone, Deserialize)]
pub struct DatabaseConfig {
    pub database_url: String,
    pub max_connections: u32
}

/// 初始化数据库连接
///
/// 创建 PostgreSQL 连接池，配置最大/最小连接数、连接超时和空闲超时。
///
/// # 参数
/// - `cfg`: 应用配置，包含数据库 URL 和最大连接数
///
/// # 返回
/// 返回数据库连接 `DatabaseConnection`
///
/// # 错误
/// - `Box<dyn std::error::Error>`: 数据库连接失败时返回
pub async fn init_db(cfg: &AppConfig) -> Result<DatabaseConnection, Box<dyn std::error::Error>> {
    let mut opt = ConnectOptions::new(cfg.database_config.database_url.clone());
    opt
        .max_connections(cfg.database_config.max_connections)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(30));

    Ok(Database::connect(opt).await?)
}
