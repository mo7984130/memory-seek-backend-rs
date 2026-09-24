//! 缓存能力:Redis 连接池与扩展 trait。

pub type Pool = deadpool_redis::Pool;

mod redis_ext;
pub use redis_ext::*;
