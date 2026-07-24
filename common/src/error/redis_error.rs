use crate::{error::AppError, ext::log_err_with_err};

impl From<deadpool_redis::PoolError> for AppError {
    #[track_caller]
    fn from(value: deadpool_redis::PoolError) -> Self {
        log_err_with_err(
            "redis_err",
            "Redis错误",
            &value,
            AppError::InternalServerError,
        )
    }
}

impl From<redis::RedisError> for AppError {
    fn from(value: redis::RedisError) -> Self {
        log_err_with_err(
            "redis_err",
            "Redis错误",
            &value,
            AppError::InternalServerError,
        )
    }
}
