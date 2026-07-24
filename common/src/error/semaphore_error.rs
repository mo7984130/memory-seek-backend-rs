use crate::error::AppError;
use crate::ext::log_err_with_err;
use tokio::sync::AcquireError;

impl From<AcquireError> for AppError {
    fn from(value: AcquireError) -> Self {
        log_err_with_err(
            "semmaphore_error",
            "信号量错误",
            &value,
            AppError::InternalServerError,
        )
    }
}
