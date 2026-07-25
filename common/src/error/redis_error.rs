use std::fmt::Debug;

use crate::{
    error::AppError,
    ext::{TraceExt, log_err_with_err},
};

#[track_caller]
fn log_redis_err(value: impl Debug) -> AppError {
    log_err_with_err(
        "redis_err",
        "Redis错误",
        &value,
        AppError::InternalServerError,
    )
}

impl From<deadpool_redis::PoolError> for AppError {
    #[track_caller]
    fn from(value: deadpool_redis::PoolError) -> Self {
        log_redis_err(value)
    }
}

impl From<redis::RedisError> for AppError {
    #[track_caller]
    fn from(value: redis::RedisError) -> Self {
        log_redis_err(value)
    }
}

impl<T> TraceExt<T> for Result<T, deadpool_redis::PoolError> {
    #[track_caller]
    fn trace(self) -> Result<T, AppError> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(log_redis_err(e)),
        }
    }
}
impl<T> TraceExt<T> for Result<T, redis::RedisError> {
    fn trace(self) -> Result<T, AppError> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(log_redis_err(e)),
        }
    }
}
