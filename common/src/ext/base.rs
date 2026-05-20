use crate::error::{AppError, DbErrExt};
use std::fmt::Debug;
use tracing::{error, warn};

pub fn log_internal_err<E>(reason: &'static str, context: &'static str, e: E) -> AppError
where
    E: Debug,
{
    error!(%reason, status = "error", error = ?e, "{context}");
    AppError::InternalServerError
}

pub fn log_bad_request_warn<E>(reason: &'static str, msg: &'static str, e: E) -> AppError
where
    E: Debug,
{
    warn!(%reason, status = "failed", error = ?e, "{msg}");
    AppError::bad_request(msg)
}

pub fn log_conflict_warn<E>(reason: &'static str, msg: &'static str, e: E) -> AppError
where
    E: Debug + DbErrExt,
{
    warn!(%reason, status = "failed", error = ?e, "{msg}");
    AppError::Conflict(msg.into())
}
