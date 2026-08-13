use std::fmt::Debug;

use crate::error::{AppError, ContextualError, contextual::Result};

/// 使用按 feature 开启的 `From<E> for ContextualError` 显式延迟错误。
pub trait IntoContextualExt<T> {
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
    fn context_error(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T>;

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
    fn context_warn_none(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T>;

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
    fn emit_if_err(self);
}

impl<T> ContextualResultExt<T> for Result<T> {
    fn emit_if_err(self) {
        if let Err(error) = self {
            error.emit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ContextualResultExt;

    #[test]
    fn emit_if_err_accepts_successful_result() {
        Ok::<(), crate::error::ContextualError>(()).emit_if_err();
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
