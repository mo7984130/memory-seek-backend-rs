use common::ext::ResultErrExt;
use serde::Deserialize;

use deadpool_redis::{Config as DeadpoolConfig, Pool, PoolConfig, Runtime};
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_url")]
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

fn default_url() -> String {
    "redis://localhost:6379".to_string()
}
const fn default_max_connections() -> u32 {
    16
}

pub fn init(cfg: &Config) -> Result<Pool, common::error::AppError> {
    info!("初始化 Redis");
    let mut redis_cfg = DeadpoolConfig::from_url(&cfg.url);
    redis_cfg.pool = Some(PoolConfig::new(cfg.max_connections as usize));
    let pool = redis_cfg
        .create_pool(Some(Runtime::Tokio1))
        .trace_internal_err("redis_pool_err", "Redis连接池创建失败")?;
    info!("Redis 连接成功, max_connections: {}", cfg.max_connections);
    Ok(pool)
}
