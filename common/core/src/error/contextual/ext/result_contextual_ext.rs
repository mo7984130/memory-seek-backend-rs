use std::fmt::Debug;

use crate::error::{AppError, ContextualError, ContextualResult};

pub trait ResultContextualExt<T> {
    fn context_err(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> ContextualResult<T>;

    fn context_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> ContextualResult<T>;
}

impl<T, E> ResultContextualExt<T> for std::result::Result<T, E>
where
    E: Debug + Send + Sync + 'static,
{
    fn context_err(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> ContextualResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(source) => Err(ContextualError::error(reason, context, source, app_error)),
        }
    }

    fn context_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> ContextualResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(source) => Err(ContextualError::warn(reason, context, source, app_error)),
        }
    }
}
