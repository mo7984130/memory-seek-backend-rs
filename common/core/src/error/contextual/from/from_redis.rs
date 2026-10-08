use crate::error::{AppError, ContextualError};

impl From<deadpool_redis::PoolError> for ContextualError {
    /// 将 Redis 连接池错误转换为上下文错误
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
    /// 将 `redis::RedisError` 转换为上下文错误
    fn from(error: redis::RedisError) -> Self {
        Self::error(
            "redis_err",
            "Redis错误",
            error,
            AppError::InternalServerError,
        )
    }
}
