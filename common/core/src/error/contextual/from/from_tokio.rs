use crate::error::{AppError, ContextualError};

impl From<tokio::sync::AcquireError> for ContextualError {
    fn from(error: tokio::sync::AcquireError) -> Self {
        Self::error(
            "tokio_semaphore_error",
            "信号量错误",
            error,
            AppError::InternalServerError,
        )
    }
}

impl From<tokio::task::JoinError> for ContextualError {
    fn from(error: tokio::task::JoinError) -> Self {
        Self::error(
            "tokio_join_error",
            "Tokio 任务执行失败",
            error,
            AppError::InternalServerError,
        )
    }
}
