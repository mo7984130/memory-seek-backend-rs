use std::fmt;

use base64::DecodeError;

use crate::error::AppError;
use crate::ext::log_warn_with_err;

/// 游标解码失败
///
/// 作为 `TimeIdCursor::decode` 的专有错误类型，自动通过 `From` 转换为 `AppError::BadRequest`。
#[derive(Debug)]
pub enum CursorDecodeError {
    /// Base64 解码失败
    Base64(DecodeError),
    /// UTF-8 解析失败
    Utf8(std::string::FromUtf8Error),
    /// JSON 解析失败
    Json(serde_json::Error),
}

impl fmt::Display for CursorDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl From<CursorDecodeError> for AppError {
    #[track_caller]
    fn from(e: CursorDecodeError) -> Self {
        log_warn_with_err(
            "cursor_decode_error",
            "游标解码失败",
            e,
            AppError::bad_request("游标解析失败"),
        )
    }
}
