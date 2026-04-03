use crate::error::AppError;
use tracing::{warn, Span};

pub trait BoolExt {
    fn ok_or_warn(self, reason: &'static str, msg: &'static str) -> Result<(), AppError>;

    fn ok_else_warn<F>(self, reason: &'static str, msg: &'static str, f: F) -> Result<(), AppError>
    where
        F: FnOnce(&Span);
}

impl BoolExt for bool {
    fn ok_or_warn(self, reason: &'static str, msg: &'static str) -> Result<(), AppError> {
        if self {
            Ok(())
        } else {
            let span = Span::current();
            span.record("status", "failed");
            span.record("reason", reason);

            warn!(%reason, "{msg}");
            Err(AppError::bad_request(msg))
        }
    }

    fn ok_else_warn<F>(self, reason: &'static str, msg: &'static str, f: F) -> Result<(), AppError>
    where
        F: FnOnce(&Span),
    {
        if self {
            Ok(())
        } else {
            let span = Span::current();
            f(&span);

            warn!(%reason, status="failed", "{msg}");
            Err(AppError::bad_request(msg))
        }
    }
}