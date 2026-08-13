use std::fmt::Debug;

use crate::error::{AppError, DeferredError, DeferredResult};

pub trait DeferResultExt<T> {
    fn defer_error(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> DeferredResult<T>;

    fn defer_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> DeferredResult<T>;
}

impl<T, E> DeferResultExt<T> for std::result::Result<T, E>
where
    E: Debug + Send + Sync + 'static,
{
    fn defer_error(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> DeferredResult<T> {
        self.map_err(|error| DeferredError::error(reason, context, error, app_error))
    }

    fn defer_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> DeferredResult<T> {
        self.map_err(|error| DeferredError::warn(reason, context, error, app_error))
    }
}

pub trait DeferOptionExt<T> {
    fn defer_warn_none(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> DeferredResult<T>;

    fn defer_error_none(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> DeferredResult<T>;
}

impl<T> DeferOptionExt<T> for Option<T> {
    fn defer_warn_none(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> DeferredResult<T> {
        self.ok_or_else(|| DeferredError::warn_without_source(reason, context, app_error))
    }

    fn defer_error_none(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> DeferredResult<T> {
        self.ok_or_else(|| DeferredError::error_without_source(reason, context, app_error))
    }
}
