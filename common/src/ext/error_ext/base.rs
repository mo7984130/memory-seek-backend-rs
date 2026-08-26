use crate::error::AppError;
use std::fmt::Debug;
use std::panic::Location;
use tracing::Level;

#[track_caller]
pub fn log_and_map(
    level: Level,
    reason: &'static str,
    context: &'static str,
    err: Option<&dyn Debug>,
    app_err: AppError,
) -> AppError {
    let loc = Location::caller();
    log_and_map_at(level, reason, context, err, app_err, loc)
}

/// 在调用点记录错误上下文并映射为应用错误.
pub(crate) fn log_and_map_at(
    level: Level,
    reason: &'static str,
    context: &'static str,
    err: Option<&dyn Debug>,
    app_err: AppError,
    loc: &'static Location<'static>,
) -> AppError {
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

    app_err
}

macro_rules! define_log_fns {
    ($name:ident, $name_with_source:ident, $level:expr) => {
        #[track_caller]
        pub fn $name(reason: &'static str, context: &'static str, app_err: AppError) -> AppError {
            log_and_map($level, reason, context, None, app_err)
        }

        #[track_caller]
        pub fn $name_with_source(
            reason: &'static str,
            context: &'static str,
            source: impl Debug,
            app_err: AppError,
        ) -> AppError {
            log_and_map(
                $level,
                reason,
                context,
                Some(&source as &dyn Debug),
                app_err,
            )
        }
    };
}

define_log_fns!(log_err, log_err_with_source, Level::ERROR);
define_log_fns!(log_warn, log_warn_with_source, Level::WARN);
define_log_fns!(log_info, log_info_with_source, Level::INFO);
define_log_fns!(log_debug, log_debug_with_source, Level::DEBUG);
define_log_fns!(log_trace, log_trace_with_source, Level::TRACE);
