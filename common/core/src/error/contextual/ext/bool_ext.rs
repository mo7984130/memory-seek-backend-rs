use std::borrow::Cow;

use crate::error::{ContextualError, ContextualResult};

/// 为 `bool` 提供条件校验便捷方法
pub trait BoolExt {
    /// 为真返回 `Ok`，否则返回 warn 级上下文错误
    fn true_or_warn<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<(), E>;

    /// 为假返回 `Ok`，否则返回 warn 级上下文错误
    fn false_or_warn<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<(), E>;
}

impl BoolExt for bool {
    /// 为真返回 `Ok`，否则返回 warn 级上下文错误
    fn true_or_warn<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<(), E> {
        if self {
            Ok(())
        } else {
            Err(ContextualError::warn_without_source(
                reason, context, payload,
            ))
        }
    }

    /// 为假返回 `Ok`，否则返回 warn 级上下文错误
    fn false_or_warn<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<(), E> {
        if self {
            Err(ContextualError::warn_without_source(
                reason, context, payload,
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;

    #[test]
    fn true_true_or_warn_returns_ok() {
        let result = true.true_or_warn(
            "test_reason",
            "test_context",
            AppError::BadRequest("bad".into()),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn false_true_or_warn_returns_err() {
        let result = false.true_or_warn(
            "test_reason",
            "test_context",
            AppError::BadRequest("bad".into()),
        );
        assert!(result.is_err());
    }
}
