//! Redis 访问层:连接池与常用命令扩展。
//!
//! 本 crate 只提供 **Redis 实现**(连接池别名 + 命令包装),不做后端抽象;
//! 多层缓存策略(L1 内存 / L2 Redis)由 `multi-level-cache` 承担。

/// Redis 连接池(与 `deadpool_redis::Pool` 同一类型)。
pub type Pool = deadpool_redis::Pool;

mod redis_ext;
pub use redis_ext::*;
