use crate::error::AppError;
use tracing::{warn, Span};

/// 为 `bool` 提供条件校验便捷方法
pub trait BoolExt {
    /// 当值为 `true` 时返回 `Ok(())`，否则记录 `warn!` 日志并返回 `AppError::BadRequest`
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `msg`: `BadRequest` 错误消息
    ///
    /// # 返回
    /// `true` 时返回 `Ok(())`，`false` 时返回 `AppError::BadRequest(msg)`
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 值为 `false` 时
    fn ok_or_warn(self, reason: &'static str, msg: &'static str) -> Result<(), AppError>;

    /// 当值为 `true` 时返回 `Ok(())`，否则在当前 `Span` 上执行闭包 `f` 后记录 `warn!` 日志并返回 `AppError::BadRequest`
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `msg`: `BadRequest` 错误消息
    /// - `f`: 失败时对当前 `Span` 执行的回调，可用于附加额外字段
    ///
    /// # 返回
    /// `true` 时返回 `Ok(())`，`false` 时返回 `AppError::BadRequest(msg)`
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 值为 `false` 时
    fn ok_else_warn<F>(self, reason: &'static str, msg: &'static str, f: F) -> Result<(), AppError>
    where
        F: FnOnce(&Span);
}

impl BoolExt for bool {
    /// 当值为 `true` 时返回 `Ok(())`，否则记录 `warn!` 日志并返回 `AppError::BadRequest`
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `msg`: `BadRequest` 错误消息
    ///
    /// # 返回
    /// `true` 时返回 `Ok(())`，`false` 时返回 `AppError::BadRequest(msg)`
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 值为 `false` 时
    fn ok_or_warn(self, reason: &'static str, msg: &'static str) -> Result<(), AppError> {
        if self {
            Ok(())
        } else {
            warn!(%reason, status="failed", "{msg}");
            Err(AppError::bad_request(msg))
        }
    }

    /// 当值为 `true` 时返回 `Ok(())`，否则在当前 `Span` 上执行闭包 `f` 后记录 `warn!` 日志并返回 `AppError::BadRequest`
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `msg`: `BadRequest` 错误消息
    /// - `f`: 失败时对当前 `Span` 执行的回调，可用于附加额外字段
    ///
    /// # 返回
    /// `true` 时返回 `Ok(())`，`false` 时返回 `AppError::BadRequest(msg)`
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 值为 `false` 时
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
