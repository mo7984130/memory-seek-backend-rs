use crate::error::{AppError, ContextualError};

impl From<multi_level_cache::CacheError> for ContextualError {
    fn from(error: multi_level_cache::CacheError) -> Self {
        Self::warn(
            "cache_err",
            "缓存错误",
            error,
            AppError::InternalServerError,
        )
    }
}
