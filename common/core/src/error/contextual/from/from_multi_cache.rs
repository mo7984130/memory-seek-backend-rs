use crate::error::{AppError, ContextualError};

impl From<multi_level_cache::CacheError> for ContextualError {
    /// 将多级缓存错误转换为 warn 级上下文错误
    fn from(error: multi_level_cache::CacheError) -> Self {
        Self::warn(
            "cache_err",
            "缓存错误",
            error,
            AppError::InternalServerError,
        )
    }
}
