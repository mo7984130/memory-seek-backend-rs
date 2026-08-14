use common::{Result, error::ContextualError};
use serde::Deserialize;

use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}
const fn default_max_connections() -> u32 {
    64
}

/// 根据配置建立数据库连接并执行基础初始化.
pub async fn init(cfg: &Config) -> Result<DatabaseConnection> {
    info!("初始化数据库");
    let mut opt = ConnectOptions::new(&cfg.url);
    opt.max_connections(cfg.max_connections);
    let db = Database::connect(opt).await.map_err(|source| {
        ContextualError::error(
            "db_connect_err",
            "数据库连接失败",
            source,
            common::error::AppError::InternalServerError,
        )
    })?;
    info!("数据库连接成功, max_connections: {}", cfg.max_connections);
    Ok(db)
}
