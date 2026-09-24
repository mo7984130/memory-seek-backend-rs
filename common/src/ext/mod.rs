//! 扩展 trait 门面。
//!
//! `apply_ext` 与 `error_ext` 在 `common-core`,`redis_ext` 在 `common-cache`,
//! 此处合并重导出以保持 `common::ext::{ToOk, RedisExt, …}` 路径不变。

pub use common_cache::*;
pub use common_core::ext::*;
