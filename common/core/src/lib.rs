//! 公共基础能力:统一错误、时间、扩展 trait 与通用值类型。
//!
//! 这是所有其它能力 crate 与域契约 crate 的共同底座, 因此保持最稳定、最少的直接依赖:
//! - [`error`]：`AppError` / `ContextualError` 与各外部错误类型的 `From` 实现
//!   (孤儿规则决定它们必须与错误类型同 crate, 故 sea-orm / redis / axum / tokio
//!   的转换 impl 也在这里; axum 响应 impl 在需要时由上层 crate 提供)
//! - [`time`]：`DateTime` / `now` / `after`
//! - [`ext`]：`Result` / `Option` 组合辅助 trait
//! - [`types`]：`HasChanged`、`CursorPage` 等通用值类型
//! - [`snowflake`]：分布式有序 ID 生成(`id-gen` feature)

pub mod error;
pub mod ext;
pub mod time;
pub mod types;

/// 分布式有序 ID 生成(雪花算法,`id-gen` feature)。
///
/// 测试构建下也会编译(`cfg(test)`),以便单测不受 feature 开关影响。
#[cfg(any(test, feature = "id-gen"))]
pub mod snowflake;

/// 统一 API 响应格式 `R`(axum 适配层共用)。
#[cfg(feature = "axum")]
pub mod r;

pub use error::{AppError, ContextualError, ContextualResult, Result};

/// sea-orm 连接抽象的重导出(实体与仓储代码常用别名)。
pub use sea_orm::ConnectionTrait as DbConn;

mod temp_file;
mod type_map;

pub use temp_file::{TempFile, remove_dir_all};
pub use type_map::TypeMap;
