use crate::error::{AppError, ContextualError};

impl From<deadpool_redis::PoolError> for ContextualError {
    fn from(error: deadpool_redis::PoolError) -> Self {
        Self::error(
            "redis_err",
            "Redis错误",
            error,
            AppError::InternalServerError,
        )
    }
}

impl From<redis::RedisError> for ContextualError {
    fn from(error: redis::RedisError) -> Self {
        Self::error(
            "redis_err",
            "Redis错误",
            error,
            AppError::InternalServerError,
        )
    }
}
