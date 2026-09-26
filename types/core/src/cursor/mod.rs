//! 共享内核 `types-core` 的游标编解码:key 集分页的游标契约。
//!
//! 被 audit / visual 等上下文的 DTO 与查询层共同引用,因此位于各域契约 crate 之下。
//! `orm` feature 下提供 `keyset_condition` / `before` / `after` 等键集查询辅助,
//! 并将 [`CursorDecodeError`] 转换为 `AppError`。
//!
//! 每个游标一个文件,两者共用的部件留在本模块:
//! - `time_id`: `(time, id)` 复合游标
//! - `count_id`: `(count, id)` 复合游标,用于按计数主排序的 keyset 分页
//! - [`KeysetDirection`]: 排序方向,与查询的 `ORDER BY` 配套
//! - [`CursorDecodeError`]: 两者 `decode` 共用的失败类型

use base64::DecodeError;

#[cfg(feature = "orm")]
use common_core::error::{AppError, ContextualError};

mod count_id;
mod time_id;

pub use count_id::*;
pub use time_id::*;

#[cfg(test)]
mod tests;

/// keyset 分页排序方向, 需与查询的 `ORDER BY` 保持一致
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeysetDirection {
    /// 倒序: `ORDER BY time DESC, id DESC`, 游标取 `(time, id) < cursor`
    Desc,
    /// 正序: `ORDER BY time ASC, id ASC`, 游标取 `(time, id) > cursor`
    Asc,
}

/// 游标解码失败
///
/// 作为 [`TimeIdCursor::decode`] / [`CountIdCursor::decode`] 的专有错误类型，
/// 后端 orm 模式下自动通过 `From` 转换为 `AppError::BadRequest`。
#[derive(Debug)]
pub enum CursorDecodeError {
    /// Base64 解码失败
    Base64(DecodeError),
    /// UTF-8 解析失败
    Utf8(std::string::FromUtf8Error),
    /// JSON 解析失败
    Json(serde_json::Error),
}

impl std::fmt::Display for CursorDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Base64(_) => write!(f, "游标 Base64 解码失败"),
            Self::Utf8(_) => write!(f, "游标 UTF-8 解析失败"),
            Self::Json(_) => write!(f, "游标 JSON 解析失败"),
        }
    }
}

impl std::error::Error for CursorDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Base64(e) => Some(e),
            Self::Utf8(e) => Some(e),
            Self::Json(e) => Some(e),
        }
    }
}

#[cfg(feature = "orm")]
impl From<CursorDecodeError> for AppError {
    #[track_caller]
    fn from(e: CursorDecodeError) -> Self {
        ContextualError::warn(
            "cursor_decode_error",
            "游标解码失败",
            e,
            AppError::bad_request("游标解析失败"),
        )
        .emit()
    }
}
