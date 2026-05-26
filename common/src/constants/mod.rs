//! 全局常量定义
//!
//! 包含 Redis 缓存键生成函数（[`redis_keys`]）、密码哈希器（[`HASHER`]）和密码验证并发度配置。

pub mod redis_keys;
pub use redis_keys as RedisKeys;

mod password_hasher;
pub use password_hasher::HASHER as PasswordHasher;
