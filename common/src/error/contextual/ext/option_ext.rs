use crate::{
    error::{AppError, ContextualError, ContextualResult},
    ext::ToErr,
};

pub trait OptionExt<T> {
    fn ok_or_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> ContextualResult<T>;

    fn ok_or_error(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> ContextualResult<T>;
}

impl<T> OptionExt<T> for Option<T> {
    #[inline]
    fn ok_or_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> ContextualResult<T> {
        match self {
            Some(v) => Ok(v),
            None => ContextualError::warn_without_source(reason, context, app_err).to_err(),
        }
    }

    #[inline]
    fn ok_or_error(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> ContextualResult<T> {
        match self {
            Some(v) => Ok(v),
            None => ContextualError::error_without_source(reason, context, app_err).to_err(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
