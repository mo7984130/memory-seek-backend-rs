//! Redis 缓存键生成函数
//!
//! 提供各业务域的 Redis 键命名规则，避免硬编码分散在业务代码中。

pub mod user;

pub mod photo;