//! 公共基础能力:统一错误、时间、扩展 trait 与通用值类型。
//!
//! 这是所有其它能力 crate 与域契约 crate 的共同底座, 因此保持最稳定、最少的直接依赖:
//! - [`error`]：`AppError` / `ContextualError` 与各外部错误类型的 `From` 实现
//!   (孤儿规则决定它们必须与错误类型同 crate, 故 sea-orm / redis / axum / tokio
//!   的转换 impl 也在这里; axum 响应 impl 在需要时由上层 crate 提供)
//! - [`time`]：`DateTime` / `now` / `after`
//! - [`ext`]：`Result` / `Option` 组合辅助 trait
//! - [`types`]：`HasChanged`、`CursorPage` 等通用值类型

pub mod error;
pub mod ext;
pub mod time;
pub mod types;

/// 统一 API 响应格式 `R`(axum 适配层共用)。
#[cfg(feature = "axum")]
pub mod r;

pub use error::{AppError, ContextualError, ContextualResult, Result};
