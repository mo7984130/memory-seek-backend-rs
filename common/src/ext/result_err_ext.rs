use crate::{
    error::AppError,
    ext::{log_err, log_warn},
};
use std::fmt::{Debug, Display};

// ============================================================
// ResultErrExt
// ============================================================

pub trait ResultErrExt<T, E>: Sized {
    fn trace_err(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError>;

    fn trace_internal_err(self, reason: &'static str, context: &'static str) -> Result<T, AppError>;

    fn trace_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError>;

    fn trace_warn_bad_request(
        self,
        reason: &'static str,
        context: &'static str,
        msg: &'static str,
    ) -> Result<T, AppError>;
}

impl<T, E: Debug + Display> ResultErrExt<T, E> for Result<T, E> {
    fn trace_err(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError> {
        self.map_err(|e| log_err(reason, context, e, app_err))
    }

    fn trace_internal_err(self, reason: &'static str, context: &'static str) -> Result<T, AppError> {
        self.trace_err(reason, context, AppError::InternalServerError)
    }

    fn trace_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError> {
        self.map_err(|e| log_warn(reason, context, e, app_err))
    }

    fn trace_warn_bad_request(
        self,
        reason: &'static str,
        context: &'static str,
        msg: &'static str,
    ) -> Result<T, AppError> {
        self.trace_warn(reason, context, AppError::bad_request(msg))
    }
}
