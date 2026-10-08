use std::borrow::Cow;
use std::fmt::Debug;

use crate::error::{AppError, ContextualError, ContextualResult};

pub trait ResultExt<T> {
    /// 为 `Ok` 返回值，`Err` 时包装为 error 级上下文错误
    fn context_error(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> ContextualResult<T>;

    /// 为 `Ok` 返回值，`Err` 时包装为 warn 级上下文错误
    fn context_warn(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> ContextualResult<T>;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: Debug + Send + Sync + 'static,
{
    /// 为 `Ok` 返回值，`Err` 时包装为 error 级上下文错误
    fn context_error(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> ContextualResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(source) => Err(ContextualError::error(reason, context, source, app_error)),
        }
    }

    /// 为 `Ok` 返回值，`Err` 时包装为 warn 级上下文错误
    fn context_warn(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> ContextualResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(source) => Err(ContextualError::warn(reason, context, source, app_error)),
        }
    }
}
