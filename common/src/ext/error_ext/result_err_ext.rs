use crate::{
    error::AppError,
    ext::error_ext::{log_err_with_err, log_warn_with_err},
};
use std::fmt::Debug;

pub trait TraceExt<T> {
    #[track_caller]
    fn trace(self) -> Result<T, AppError>;
}

// ============================================================
// ResultErrExt
// ============================================================

pub trait ResultErrExt<T, E>: Sized {
    fn trace_err(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError>;

    fn trace_internal_err(self, reason: &'static str, context: &'static str)
    -> Result<T, AppError>;

    fn trace_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError>;

    fn trace_warn_bad_request(
        self,
        reason: &'static str,
        context: &'static str,
        msg: &'static str,
    ) -> Result<T, AppError>;
}

impl<T, E: Debug> ResultErrExt<T, E> for Result<T, E> {
    #[track_caller]
    fn trace_err(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError> {
        self.map_err(|e| log_err_with_err(reason, context, e, app_err))
    }

    #[track_caller]
    fn trace_internal_err(
        self,
        reason: &'static str,
        context: &'static str,
    ) -> Result<T, AppError> {
        self.trace_err(reason, context, AppError::InternalServerError)
    }

    #[track_caller]
    fn trace_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError> {
        self.map_err(|e| log_warn_with_err(reason, context, e, app_err))
    }

    #[track_caller]
    fn trace_warn_bad_request(
        self,
        reason: &'static str,
        context: &'static str,
        msg: &'static str,
    ) -> Result<T, AppError> {
        self.trace_warn(reason, context, AppError::bad_request(msg))
    }
}

pub trait ResultInspectErrAsync<T, E> {
    #[allow(async_fn_in_trait)]
    async fn inspect_err_async<F, Fut>(self, f: F) -> Result<T, E>
    where
        F: FnOnce(&E) -> Fut + Send,
        Fut: std::future::Future<Output = ()> + Send;
}

impl<T: Send, E: Send> ResultInspectErrAsync<T, E> for Result<T, E> {
    async fn inspect_err_async<F, Fut>(self, f: F) -> Result<T, E>
    where
        F: FnOnce(&E) -> Fut + Send,
        Fut: std::future::Future<Output = ()> + Send,
    {
        if let Err(e) = &self {
            f(e).await;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_trace_err_returns_value() {
        let result = Ok::<i32, String>(42).trace_err(
            "test_reason",
            "test_context",
            AppError::BadRequest("custom".into()),
        );
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn err_trace_err_returns_custom_app_err() {
        let result = Err::<i32, String>("e".into()).trace_err(
            "test_reason",
            "test_context",
            AppError::BadRequest("custom".into()),
        );
        assert!(matches!(result.unwrap_err(), AppError::BadRequest(_)));
    }

    #[test]
    fn ok_trace_internal_err_returns_value() {
        let result = Ok::<i32, String>(42).trace_internal_err("test_reason", "test_context");
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn err_trace_internal_err_returns_internal_server_error() {
        let result =
            Err::<i32, String>("e".into()).trace_internal_err("test_reason", "test_context");
        assert!(matches!(result.unwrap_err(), AppError::InternalServerError));
    }

    #[test]
    fn ok_trace_warn_returns_value() {
        let result = Ok::<i32, String>(42).trace_warn(
            "test_reason",
            "test_context",
            AppError::BadRequest("custom".into()),
        );
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn err_trace_warn_returns_custom_app_err() {
        let result = Err::<i32, String>("e".into()).trace_warn(
            "test_reason",
            "test_context",
            AppError::BadRequest("custom".into()),
        );
        assert!(matches!(result.unwrap_err(), AppError::BadRequest(_)));
    }
}
