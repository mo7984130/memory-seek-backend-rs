use std::{fmt::Debug, future::Future};

use crate::error::{AppError, ContextualError, contextual::Result};

/// 使用按 feature 开启的 `From<E> for ContextualError` 显式延迟错误。
pub trait IntoContextualExt<T> {
    /// 将基础错误延迟转换为上下文错误.
    fn into_contextual(self) -> Result<T>;
}

impl<T, E> IntoContextualExt<T> for std::result::Result<T, E>
where
    ContextualError: From<E>,
{
    fn into_contextual(self) -> Result<T> {
        self.map_err(ContextualError::from)
    }
}

pub trait ContextResultExt<T> {
    /// 为错误附加 ERROR 级别的上下文.
    fn context_error(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T>;

    /// 为错误附加 WARN 级别的上下文.
    fn context_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T>;
}

impl<T, E> ContextResultExt<T> for std::result::Result<T, E>
where
    E: Debug + Send + Sync + 'static,
{
    fn context_error(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T> {
        self.map_err(|error| ContextualError::error(reason, context, error, app_error))
    }

    fn context_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T> {
        self.map_err(|error| ContextualError::warn(reason, context, error, app_error))
    }
}

pub trait ContextOptionExt<T> {
    /// 将 None 转换为带 WARN 上下文的错误.
    fn context_warn_none(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T>;

    /// 将 None 转换为带 ERROR 上下文的错误.
    fn context_error_none(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T>;
}

/// 消费上下文化结果；若为错误则记录其上下文。
///
/// 适用于补偿或清理操作失败时只需记录、不能覆盖原始错误的场景。
pub trait ContextualResultExt<T> {
    /// 记录错误上下文并丢弃错误结果.
    fn emit_if_err(self);
}

impl<T> ContextualResultExt<T> for Result<T> {
    fn emit_if_err(self) {
        if let Err(error) = self {
            error.emit();
        }
    }
}

/// 缓存基础设施不可用时回源加载，业务错误仍按原样返回。
pub async fn fallback_on_cache_error<T, F, Fut>(result: Result<T>, loader: F) -> Result<T>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    match result {
        Ok(value) => Ok(value),
        Err(error) if error.reason() == "cache_err" => {
            error.emit();
            loader().await
        }
        Err(error) => Err(error),
    }
}

impl<T> ContextOptionExt<T> for Option<T> {
    fn context_warn_none(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T> {
        self.ok_or_else(|| ContextualError::warn_without_source(reason, context, app_error))
    }

    fn context_error_none(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T> {
        self.ok_or_else(|| ContextualError::error_without_source(reason, context, app_error))
    }
}

#[cfg(test)]
mod tests {
    use super::{ContextualResultExt, fallback_on_cache_error};
    use crate::error::{AppError, ContextualError};

    #[test]
    fn emit_if_err_accepts_successful_result() {
        Ok::<(), crate::error::ContextualError>(()).emit_if_err();
    }

    #[tokio::test]
    async fn cache_error_falls_back_to_loader() {
        let result = fallback_on_cache_error(
            Err(ContextualError::warn_without_source(
                "cache_err",
                "缓存错误",
                AppError::InternalServerError,
            )),
            || async { Ok(42) },
        )
        .await;

        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn business_error_does_not_fall_back_to_loader() {
        let result = fallback_on_cache_error(
            Err(ContextualError::warn_without_source(
                "record_not_found",
                "记录不存在",
                AppError::bad_request("记录不存在"),
            )),
            || async { Ok(42) },
        )
        .await;

        assert_eq!(result.unwrap_err().reason(), "record_not_found");
    }
}
