//! 扩展 trait 门面。
//!
//! `apply_ext` 与 `error_ext` 已下沉 `common-core`,此处合并重导出;
//! `redis_ext` 仍属本 crate 的缓存能力。

pub use common_core::ext::*;

mod redis_ext;
pub use redis_ext::*;
