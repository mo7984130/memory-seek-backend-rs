use common::{Result, error::ContextualError};
use serde::Deserialize;

use crate::{config::AppConfig, setup::AppSetup};
use sea_orm::{ConnectOptions, Database};
use tracing::{debug, info};

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
#[common::register_async(
    slice = crate::setup::bases::APP_BASES,
    ty = crate::setup::InitFn,
)]
pub async fn init(config: &AppConfig, setup: &mut AppSetup) -> Result<()> {
    debug!("初始化数据库");
    let cfg = &config.database;
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

    types::db_init::init_db(&db).await?;

    setup.registry.insert(db);

    info!("数据库连接成功");
    Ok(())
}
