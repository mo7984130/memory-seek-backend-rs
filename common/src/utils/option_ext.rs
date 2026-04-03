use crate::error::AppError;
use tracing::{error, warn, Span};

pub trait OptionExt<T> {
    fn ok_or_warn(self, reason: &'static str, context: &'static str) -> Result<T, AppError>;

    fn ok_or_error(self, reason: &'static str, context: &'static str) -> Result<T, AppError>;
}

impl<T> OptionExt<T> for Option<T> {
    #[inline]
    fn ok_or_warn(self, reason: &'static str, context: &'static str) -> Result<T, AppError> {
        self.ok_or_else(|| {
            let span = Span::current();
            span.record("status", "failed");
            span.record("reason", reason);

            warn!(%reason, "{context}");
            AppError::bad_request(context)
        })
    }

    #[inline]
    fn ok_or_error(self, reason: &'static str, context: &'static str) -> Result<T, AppError> {
        self.ok_or_else(|| {
            let span = Span::current();
            span.record("status", "error");
            span.record("reason", reason);

            error!(%reason, "{context}");
            AppError::InternalServerError
        })
    }
}