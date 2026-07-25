use s3::error::S3Error;
use std::fmt::Debug;

use crate::{
    error::AppError,
    ext::{TraceExt, log_err_with_err},
};

#[track_caller]
fn log_s3_error(err: impl Debug) -> AppError {
    log_err_with_err("oss_error", "Oss错误", err, AppError::InternalServerError)
}

impl From<S3Error> for AppError {
    #[track_caller]
    fn from(value: S3Error) -> Self {
        log_s3_error(value)
    }
}

impl<T> TraceExt<T> for Result<T, S3Error> {
    #[track_caller]
    fn trace(self) -> Result<T, AppError> {
        match self {
            Ok(v) => Ok(v),
            Err(err) => Err(log_s3_error(err)),
        }
    }
}
