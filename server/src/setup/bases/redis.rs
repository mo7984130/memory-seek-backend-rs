use common::ext::ResultErrExt;

use crate::config::AppConfig;
use deadpool_redis::{Config, Pool, PoolConfig, Runtime};
use tracing::info;

pub fn init(cfg: &AppConfig) -> Result<Pool, common::error::AppError> {
    info!("初始化 Redis");
    let mut redis_cfg = Config::from_url(&cfg.redis.url);
    redis_cfg.pool = Some(PoolConfig::new(cfg.redis.max_connections as usize));
    let pool = redis_cfg
        .create_pool(Some(Runtime::Tokio1))
        .trace_internal_err("redis_pool_err", "Redis连接池创建失败")?;
    info!(
        "Redis 连接成功, max_connections: {}",
        cfg.redis.max_connections
    );
    Ok(pool)
}
