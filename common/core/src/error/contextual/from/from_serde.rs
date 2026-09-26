use crate::error::{AppError, ContextualError};

impl From<serde_json::Error> for ContextualError {
    fn from(error: serde_json::Error) -> Self {
        Self::warn(
            "serde_json_error",
            "serde_json错误",
            error,
            AppError::InternalServerError,
        )
    }
}
