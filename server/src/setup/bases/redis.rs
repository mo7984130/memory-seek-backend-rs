use deadpool_redis::{Config, Pool, PoolConfig, Runtime};
use crate::config::AppConfig;

pub fn init(cfg: &AppConfig) -> anyhow::Result<Pool> {
    let mut redis_cfg = Config::from_url(&cfg.redis.url);
    redis_cfg.pool = Some(PoolConfig::new(cfg.redis.max_connections as usize));
    let pool = redis_cfg.create_pool(Some(Runtime::Tokio1))?;
    tracing::info!("Redis connected (max_connections: {})", cfg.redis.max_connections);
    Ok(pool)
}
