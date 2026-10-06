use crate::error::AppError;
use std::borrow::Cow;
use std::fmt::Debug;
use std::panic::Location;
use tracing::Level;

#[track_caller]
pub(crate) fn log(
    level: Level,
    reason: &'static str,
    context: Cow<'static, str>,
    err: Option<&(dyn Debug + Send + Sync)>,
    app_error: AppError,
) -> AppError {
    let loc = Location::caller();
    macro_rules! emit {
        ($lvl:ident) => {
            match err {
                Some(e) => tracing::$lvl!(
                    reason,
                    status = "failed",
                    error = ?e,
                    "{context} ({}:{})",
                    loc.file(),
                    loc.line()
                ),
                None => tracing::$lvl!(
                    reason,
                    status = "failed",
                    "{context} ({}:{})",
                    loc.file(),
                    loc.line()
                ),
            }
        };
    }

    match level {
        Level::ERROR => emit!(error),
        Level::WARN => emit!(warn),
        Level::INFO => emit!(info),
        Level::DEBUG => emit!(debug),
        Level::TRACE => emit!(trace),
    }

    app_error
}
