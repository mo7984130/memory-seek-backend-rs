use crate::error::{AppError, ContextualError};

impl From<serde_json::Error> for ContextualError {
    /// 将 `serde_json::Error` 转换为 warn 级上下文错误
    fn from(error: serde_json::Error) -> Self {
        Self::warn(
            "serde_json_error",
            "serde_json错误",
            error,
            AppError::InternalServerError,
        )
    }
}
