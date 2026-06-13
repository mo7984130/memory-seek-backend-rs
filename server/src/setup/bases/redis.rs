use crate::config::AppConfig;
use deadpool_redis::{Config, Pool, PoolConfig, Runtime};
use tracing::info;

pub fn init(cfg: &AppConfig) -> anyhow::Result<Pool> {
    info!("初始化 Redis");
    let mut redis_cfg = Config::from_url(&cfg.redis.url);
    redis_cfg.pool = Some(PoolConfig::new(cfg.redis.max_connections as usize));
    let pool = redis_cfg.create_pool(Some(Runtime::Tokio1))?;
    info!(
        "Redis 连接成功, max_connections: {}",
        cfg.redis.max_connections
    );
    Ok(pool)
}
