use std::borrow::Cow;

use crate::{
    error::{ContextualError, ContextualResult},
    ext::ToErr,
};

pub trait OptionExt<T> {
    /// 为 `Some` 返回值，`None` 时返回 warn 级上下文错误
    fn ok_or_warn<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<T, E>;

    /// 为 `Some` 返回值，`None` 时返回 error 级上下文错误
    fn ok_or_error<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<T, E>;
}

impl<T> OptionExt<T> for Option<T> {
    /// 为 `Some` 返回值，`None` 时返回 warn 级上下文错误
    #[inline]
    fn ok_or_warn<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<T, E> {
        match self {
            Some(v) => Ok(v),
            None => ContextualError::warn_without_source(reason, context, payload).to_err(),
        }
    }

    /// 为 `Some` 返回值，`None` 时返回 error 级上下文错误
    #[inline]
    fn ok_or_error<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<T, E> {
        match self {
            Some(v) => Ok(v),
            None => ContextualError::error_without_source(reason, context, payload).to_err(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;

    #[test]
    fn some_ok_or_warn_returns_value() {
        let result = Some(42).ok_or_warn(
            "test_reason",
            "test_context",
            AppError::BadRequest("bad".into()),
        );
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn some_ok_or_error_returns_value() {
        let result =
            Some(42).ok_or_error("test_reason", "test_context", AppError::InternalServerError);
        assert_eq!(result.unwrap(), 42);
    }
}
