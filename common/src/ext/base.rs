use crate::error::AppError;
use std::fmt::{Debug, Display};
use tracing::{debug, error, info, trace, warn};

fn log_and_map(
    level: tracing::Level,
    reason: &'static str,
    context: &'static str,
    e: impl Debug + Display,
    app_err: AppError,
) -> AppError {
    match level {
        tracing::Level::ERROR => error!(%reason, status = "failed", error = %e, "{context}"),
        tracing::Level::WARN => warn!(%reason, status = "failed", error = %e, "{context}"),
        tracing::Level::INFO => info!(%reason, status = "failed", error = %e, "{context}"),
        tracing::Level::TRACE => trace!(%reason, status = "failed", error = %e, "{context}"),
        tracing::Level::DEBUG => debug!(%reason, status = "failed", error = %e, "{context}"),
    }
    app_err
}

pub fn log_err(
    reason: &'static str,
    context: &'static str,
    e: impl Debug + Display,
    app_err: AppError,
) -> AppError {
    log_and_map(tracing::Level::ERROR, reason, context, e, app_err)
}

pub fn log_warn(
    reason: &'static str,
    context: &'static str,
    e: impl Debug + Display,
    app_err: AppError,
) -> AppError {
    log_and_map(tracing::Level::WARN, reason, context, e, app_err)
}
