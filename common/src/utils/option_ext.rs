use crate::error::AppError;
use tracing::{error, warn};

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
    fn ok_or_warn(self, reason: &'static str, context: &'static str, msg: &'static str) -> Result<T, AppError>;

    /// 将 `None` 转换为 `AppError::InternalServerError`，并通过 `tracing::error!` 记录日志
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `context`: 日志中的上下文描述
    ///
    /// # 返回
    /// `Some` 时返回内部值，`None` 时返回 `AppError::InternalServerError`
    fn ok_or_error(self, reason: &'static str, context: &'static str) -> Result<T, AppError>;
}

impl<T> OptionExt<T> for Option<T> {
    /// 将 `None` 转换为 `AppError::BadRequest`，并通过 `tracing::warn!` 记录日志
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `context`: 日志中的上下文描述
    /// - `msg`: `BadRequest` 错误消息
    ///
    /// # 返回
    /// `Some` 时返回内部值，`None` 时返回 `AppError::BadRequest(msg)`
    #[inline]
    fn ok_or_warn(self, reason: &'static str, context: &'static str, msg: &'static str) -> Result<T, AppError> {
        self.ok_or_else(|| {
            warn!(%reason, status="failed", "{context}");
            AppError::bad_request(msg)
        })
    }

    /// 将 `None` 转换为 `AppError::InternalServerError`，并通过 `tracing::error!` 记录日志
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `context`: 日志中的上下文描述
    ///
    /// # 返回
    /// `Some` 时返回内部值，`None` 时返回 `AppError::InternalServerError`
    #[inline]
    fn ok_or_error(self, reason: &'static str, context: &'static str) -> Result<T, AppError> {
        self.ok_or_else(|| {
            error!(%reason, status="failed", "{context}");
            AppError::InternalServerError
        })
    }
}