use std::borrow::Cow;
use std::fmt::Debug;

use crate::error::{ContextualError, ContextualResult};

pub trait ResultExt<T> {
    /// 为 `Ok` 返回值，`Err` 时包装为 error 级上下文错误
    fn context_error<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<T, E>;

    /// 为 `Ok` 返回值，`Err` 时包装为 warn 级上下文错误
    fn context_warn<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<T, E>;
}

impl<T, Err> ResultExt<T> for std::result::Result<T, Err>
where
    Err: Debug + Send + Sync + 'static,
{
    /// 为 `Ok` 返回值，`Err` 时包装为 error 级上下文错误
    fn context_error<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<T, E> {
        match self {
            Ok(value) => Ok(value),
            Err(source) => Err(ContextualError::error(reason, context, source, payload)),
        }
    }

    /// 为 `Ok` 返回值，`Err` 时包装为 warn 级上下文错误
    fn context_warn<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<T, E> {
        match self {
            Ok(value) => Ok(value),
            Err(source) => Err(ContextualError::warn(reason, context, source, payload)),
        }
    }
}
