use std::fmt::Debug;

use crate::error::{AppError, DeferredError, deferred::Result};

/// 使用按 feature 开启的 `From<E> for DeferredError` 显式延迟错误。
pub trait DeferFromExt<T> {
    fn defer(self) -> Result<T>;
}

impl<T, E> DeferFromExt<T> for std::result::Result<T, E>
where
    DeferredError: From<E>,
{
    fn defer(self) -> Result<T> {
        self.map_err(DeferredError::from)
    }
}

pub trait DeferResultExt<T> {
    fn defer_error(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T>;

    fn defer_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T>;
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
    ) -> Result<T> {
        self.map_err(|error| DeferredError::error(reason, context, error, app_error))
    }

    fn defer_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T> {
        self.map_err(|error| DeferredError::warn(reason, context, error, app_error))
    }
}

pub trait DeferOptionExt<T> {
    fn defer_warn_none(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T>;

    fn defer_error_none(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T>;
}

impl<T> DeferOptionExt<T> for Option<T> {
    fn defer_warn_none(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T> {
        self.ok_or_else(|| DeferredError::warn_without_source(reason, context, app_error))
    }

    fn defer_error_none(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Result<T> {
        self.ok_or_else(|| DeferredError::error_without_source(reason, context, app_error))
    }
}
