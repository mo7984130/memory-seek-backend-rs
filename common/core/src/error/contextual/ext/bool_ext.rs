use std::borrow::Cow;

use crate::error::{AppError, ContextualError, contextual::ContextualResult};

/// 为 `bool` 提供条件校验便捷方法
pub trait BoolExt {
    fn true_or_warn(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> ContextualResult<()>;

    fn false_or_warn(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> ContextualResult<()>;
}

impl BoolExt for bool {
    fn true_or_warn(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> ContextualResult<()> {
        if self {
            Ok(())
        } else {
            Err(ContextualError::warn_without_source(
                reason, context, app_error,
            ))
        }
    }

    fn false_or_warn(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> ContextualResult<()> {
        if self {
            Err(ContextualError::warn_without_source(
                reason, context, app_error,
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
