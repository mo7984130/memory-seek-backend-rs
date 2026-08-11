use std::fmt::Debug;

use multi_level_cache::CacheError;

use crate::{
    error::AppError,
    ext::{TraceExt, log_warn_with_err},
};

#[track_caller]
fn log_cache_err(err: impl Debug) -> AppError {
    log_warn_with_err("cache_err", "缓存错误", err, AppError::InternalServerError)
}

impl From<CacheError> for AppError {
    #[track_caller]
    fn from(value: CacheError) -> Self {
        log_cache_err(value)
    }
}

impl<T> TraceExt<T> for Result<T, CacheError> {
    #[track_caller]
    fn trace(self) -> Result<T, AppError> {
        match self {
            Ok(value) => Ok(value),
            Err(e) => Err(log_cache_err(e)),
        }
    }
}
