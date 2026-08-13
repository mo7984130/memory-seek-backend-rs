use base64::DecodeError;
use thiserror::Error;

#[cfg(feature = "orm")]
use common::error::AppError;
#[cfg(feature = "orm")]
use common::ext::log_warn_with_source;

/// 枚举值解析错误（轻量，不依赖 AppError）
#[derive(Error, Debug)]
#[error("无效枚举值: {0}")]
pub struct ParseEnumError(pub String);

/// ID 解析错误（轻量，不依赖 AppError）
///
/// 调用方可通过 `trace_warn_bad_request` 等方式转换为 `AppError`。
/// 后端 orm 模式下直接实现 `From<ParseIdError> for AppError` 以便 `?` 直接使用。
#[derive(Error, Debug)]
#[error("{0}")]
pub struct ParseIdError(pub &'static str);

#[cfg(feature = "orm")]
impl From<ParseIdError> for AppError {
    fn from(e: ParseIdError) -> Self {
        AppError::bad_request(e.0)
    }
}

/// 游标解码失败
///
/// 作为 `TimeIdCursor::decode` 的专有错误类型，后端 orm 模式下自动通过 `From` 转换为 `AppError::BadRequest`。
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
        log_warn_with_source(
            "cursor_decode_error",
            "游标解码失败",
            e,
            AppError::bad_request("游标解析失败"),
        )
    }
}
