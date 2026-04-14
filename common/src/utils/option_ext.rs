use crate::error::AppError;
use tracing::{error, warn};

pub trait OptionExt<T> {
    fn ok_or_warn(self, reason: &'static str, context: &'static str, msg: &'static str) -> Result<T, AppError>;

    fn ok_or_error(self, reason: &'static str, context: &'static str) -> Result<T, AppError>;
}

impl<T> OptionExt<T> for Option<T> {
    #[inline]
    fn ok_or_warn(self, reason: &'static str, context: &'static str, msg: &'static str) -> Result<T, AppError> {
        self.ok_or_else(|| {
            warn!(%reason, status="failed", "{context}");
            AppError::bad_request(msg)
        })
    }

    #[inline]
    fn ok_or_error(self, reason: &'static str, context: &'static str) -> Result<T, AppError> {
        self.ok_or_else(|| {
            error!(%reason, status="failed", "{context}");
            AppError::InternalServerError
        })
    }
}