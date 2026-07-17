pub mod database;
pub mod log;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod redis;

use crate::config::AppConfig;
use crate::state::AppBases;

pub struct AppBasesInit;

impl AppBasesInit {
    pub async fn init(cfg: &AppConfig) -> Result<AppBases, common::error::AppError> {
        // 初始化日志
        let _log_guard = log::init(&cfg.log);

        // 初始化数据库
        let db = database::init(&cfg.database).await?;

        // 初始化 Redis
        let redis = redis::init(&cfg.redis)?;

        // 初始化 Prometheus metrics exporter
        #[cfg(feature = "metrics")]
        if let Some(metrics_cfg) = &cfg.metrics {
            metrics::init(metrics_cfg);
        }

        Ok(AppBases {
            db,
            redis,
            _log_guard,
        })
    }
}
