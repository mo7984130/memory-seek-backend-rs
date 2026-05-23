use crate::{
    error::AppError,
    ext::{log_err, log_warn},
};
use std::fmt::{Debug, Display};

// ============================================================
// ResultErrExt
// ============================================================

pub trait ResultErrExt<T, E>: Sized {
    fn to_err(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError>;

    fn to_internal_err(self, reason: &'static str, context: &'static str) -> Result<T, AppError>;

    fn to_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError>;
}

impl<T, E: Debug + Display> ResultErrExt<T, E> for Result<T, E> {
    fn to_err(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError> {
        self.map_err(|e| log_err(reason, context, e, app_err))
    }

    fn to_internal_err(self, reason: &'static str, context: &'static str) -> Result<T, AppError> {
        self.to_err(reason, context, AppError::InternalServerError)
    }

    fn to_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError> {
        self.map_err(|e| log_warn(reason, context, e, app_err))
    }
}
