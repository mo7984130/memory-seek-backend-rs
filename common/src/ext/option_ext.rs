use crate::{
    error::AppError,
    ext::{log_err, log_warn},
};

/// 为 `Option<T>` 提供到 `AppError` 的便捷转换方法
pub trait OptionExt<T> {
    /// 将 `None` 转换为 `AppError::BadRequest`，并通过 `tracing::warn!` 记录日志
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `context`: 日志中的上下文描述
    /// - `msg`: `BadRequest` 错误消息
    ///
    /// # 返回
    /// `Some` 时返回内部值，`None` 时返回 `AppError::BadRequest(msg)`
    fn ok_or_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError>;

    fn ok_or_warn_bad_request(
        self,
        reason: &'static str,
        context: &'static str,
        msg: &'static str,
    ) -> Result<T, AppError>;

    /// 将 `None` 转换为 `AppError::InternalServerError`，并通过 `tracing::error!` 记录日志
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `context`: 日志中的上下文描述
    ///
    /// # 返回
    /// `Some` 时返回内部值，`None` 时返回 `AppError::InternalServerError`
    fn ok_or_error(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError>;
}

impl<T> OptionExt<T> for Option<T> {
    /// 将 `None` 转换为 `AppError::BadRequest`，并通过 `tracing::warn!` 记录日志
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `context`: 日志中的上下文描述
    ///
    /// # 返回
    /// `Some` 时返回内部值，`None` 时返回 `AppError::BadRequest(msg)`
    #[inline]
    fn ok_or_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError> {
        self.ok_or_else(|| log_warn(reason, context, "", app_err))
    }

    fn ok_or_warn_bad_request(
        self,
        reason: &'static str,
        context: &'static str,
        msg: &'static str,
    ) -> Result<T, AppError> {
        self.ok_or_else(|| log_warn(reason, context, "", AppError::bad_request(msg)))
    }

    #[inline]
    fn ok_or_error(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<T, AppError> {
        self.ok_or_else(|| log_err(reason, context, "", app_err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn some_ok_or_warn_returns_value() {
        let result = Some(42).ok_or_warn(
            "test_reason",
            "test_context",
            AppError::BadRequest("bad".into()),
        );
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn none_ok_or_warn_returns_err() {
        let result: Result<i32, AppError> = None.ok_or_warn(
            "test_reason",
            "test_context",
            AppError::BadRequest("bad".into()),
        );
        assert!(result.is_err());
    }

    #[test]
    fn some_ok_or_warn_bad_request_returns_value() {
        let result = Some(42).ok_or_warn_bad_request("test_reason", "test_context", "bad request");
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn none_ok_or_warn_bad_request_returns_bad_request() {
        let result: Result<i32, AppError> =
            None.ok_or_warn_bad_request("test_reason", "test_context", "bad request");
        assert!(matches!(result.unwrap_err(), AppError::BadRequest(_)));
    }

    #[test]
    fn some_ok_or_error_returns_value() {
        let result =
            Some(42).ok_or_error("test_reason", "test_context", AppError::InternalServerError);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn none_ok_or_error_returns_err() {
        let result: Result<i32, AppError> =
            None.ok_or_error("test_reason", "test_context", AppError::InternalServerError);
        assert!(result.is_err());
    }
}
