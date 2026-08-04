use std::sync::PoisonError;

use crate::{error::AppError, ext::log_err_with_err};

impl<T> From<PoisonError<T>> for AppError {
    fn from(value: PoisonError<T>) -> Self {
        log_err_with_err(
            "poison_error",
            "锁中毒",
            value,
            AppError::InternalServerError,
        )
    }
}
