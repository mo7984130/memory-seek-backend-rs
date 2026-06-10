use deadpool_redis::{Config, Pool, Runtime};
use serde::Deserialize;
use crate::config::AppConfig;

#[derive(Clone, Deserialize)]
pub struct RedisConfig {
    pub redis_url: String,
}

/// 初始化 Redis 连接池
///
/// 根据配置创建 Redis 连接池，使用 tokio 运行时。
///
/// # 参数
/// - `cfg`: 应用配置，包含 Redis URL
///
/// # 返回
/// 返回 Redis 连接池 `Pool`
///
/// # 错误
/// - `Box<dyn std::error::Error>`: Redis 连接池创建失败时返回
pub fn init_redis(cfg: &AppConfig) -> Result<Pool, Box<dyn std::error::Error>> {
    Ok(
        Config::from_url(cfg.redis_config.redis_url.clone())
            .create_pool(Some(Runtime::Tokio1))?
    )
}
