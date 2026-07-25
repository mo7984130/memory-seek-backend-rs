use std::fmt::Debug;

use crate::{
    error::AppError,
    ext::{TraceExt, log_warn_with_err},
};

#[track_caller]
fn log_serde_json_err(err: impl Debug) -> AppError {
    log_warn_with_err(
        "serde_json_error",
        "serde_json错误",
        err,
        AppError::InternalServerError,
    )
}

impl From<serde_json::Error> for AppError {
    #[track_caller]
    fn from(value: serde_json::Error) -> Self {
        log_serde_json_err(value)
    }
}

impl<T> TraceExt<T> for Result<T, serde_json::Error> {
    #[track_caller]
    fn trace(self) -> Result<T, AppError> {
        match self {
            Ok(value) => Ok(value),
            Err(e) => Err(log_serde_json_err(e)),
        }
    }
}
