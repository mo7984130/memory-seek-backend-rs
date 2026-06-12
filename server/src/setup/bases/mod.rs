pub mod database;
pub mod log;
pub mod redis;

use crate::config::AppConfig;
use crate::state::AppBases;

pub struct AppBasesInit;

impl AppBasesInit {
    pub async fn init(cfg: &AppConfig) -> anyhow::Result<AppBases> {
        // 初始化数据库
        let db = database::init(cfg).await?;

        // 初始化 Redis
        let redis = redis::init(cfg)?;

        Ok(AppBases { db, redis })
    }
}
