use common::{Result, error::ContextualError};
use serde::Deserialize;

use deadpool_redis::{Config as DeadpoolConfig, PoolConfig, Runtime};
use tracing::{debug, info};

use crate::{config::AppConfig, setup::AppSetup};

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_url")]
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            url: default_url(),
            max_connections: default_max_connections(),
        }
    }
}

/// 返回 Redis 默认连接地址.
fn default_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}
const fn default_max_connections() -> u32 {
    16
}

#[common::register_async(
    slice = crate::setup::bases::APP_BASES,
    ty = crate::setup::InitFn,
)]
pub async fn init(config: &AppConfig, setup: &mut AppSetup) -> Result<()> {
    debug!("初始化 Redis");
    let config = &config.redis;
    let mut redis_cfg = DeadpoolConfig::from_url(&config.url);
    redis_cfg.pool = Some(PoolConfig::new(config.max_connections as usize));
    let pool = redis_cfg
        .create_pool(Some(Runtime::Tokio1))
        .map_err(|source| {
            ContextualError::error(
                "redis_pool_err",
                "Redis连接池创建失败",
                source,
                common::error::AppError::InternalServerError,
            )
        })?;
    let mut conn = pool.get().await.map_err(|source| {
        ContextualError::error(
            "redis_conn_err",
            "无法从连接池获取连接",
            source,
            common::error::AppError::InternalServerError,
        )
    })?;

    redis::cmd("PING")
        .query_async::<String>(&mut conn)
        .await
        .map_err(|source| {
            ContextualError::error(
                "redis_ping_err",
                "Redis PING失败，连接不可用",
                source,
                common::error::AppError::InternalServerError,
            )
        })?;
    setup.registry.insert(pool);
    info!("Redis 连接成功");
    Ok(())
}
