use common::{Pool, Result, error::ContextualError};
use serde::Deserialize;

use deadpool_redis::{Config as DeadpoolConfig, PoolConfig, Runtime};
use tracing::info;

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

/// 根据配置创建 Redis 连接池.
pub fn init(cfg: &Config) -> Result<Pool> {
    info!("初始化 Redis");
    let mut redis_cfg = DeadpoolConfig::from_url(&cfg.url);
    redis_cfg.pool = Some(PoolConfig::new(cfg.max_connections as usize));
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
    info!("Redis 连接成功, max_connections: {}", cfg.max_connections);
    Ok(pool)
}
