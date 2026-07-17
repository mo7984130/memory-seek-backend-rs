use common::ext::ResultErrExt;
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

pub async fn init(cfg: &Config) -> Result<DatabaseConnection, common::error::AppError> {
    info!("初始化数据库");
    let mut opt = ConnectOptions::new(&cfg.url);
    opt.max_connections(cfg.max_connections);
    let db = Database::connect(opt)
        .await
        .trace_internal_err("db_connect_err", "数据库连接失败")?;
    info!("数据库连接成功, max_connections: {}", cfg.max_connections);
    Ok(db)
}
