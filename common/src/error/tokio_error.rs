use crate::error::AppError;
use crate::ext::log_err_with_err;
use tokio::sync::AcquireError;
use tokio::task::JoinError;

impl From<AcquireError> for AppError {
    #[track_caller]
    fn from(value: AcquireError) -> Self {
        log_err_with_err(
            "tokio_semmaphore_error",
            "信号量错误",
            &value,
            AppError::InternalServerError,
        )
    }
}

impl From<JoinError> for AppError {
    #[track_caller]
    fn from(value: JoinError) -> Self {
        log_err_with_err(
            "tokio_jon_error",
            "Tokio 任务执行失败",
            &value,
            AppError::InternalServerError,
        )
    }
}
