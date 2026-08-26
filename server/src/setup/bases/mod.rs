pub mod cache;
pub mod database;
pub mod log;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod redis;

use common::Result;

use crate::config::AppConfig;
use crate::state::AppBases;

pub struct AppBasesInit;

impl AppBasesInit {
    /// 初始化数据库, Redis, 缓存和基础监控组件.
    pub async fn init(cfg: &AppConfig) -> Result<AppBases> {
        // 初始化数据库
        let db = database::init(&cfg.database).await?;

        // 初始化 Redis
        let redis = redis::init(&cfg.redis)?;

        // 初始化 Prometheus metrics recorder
        #[cfg(feature = "metrics")]
        let metrics_handle = metrics::init(&cfg.metrics);

        Ok(AppBases {
            db,
            redis,
            #[cfg(feature = "metrics")]
            metrics_handle,
        })
    }
}
