use crate::{error::AppError, ext::log_warn};

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
    fn ok_or_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<(), AppError>;
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
    fn ok_or_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_err: AppError,
    ) -> Result<(), AppError> {
        if self {
            Ok(())
        } else {
            Err(log_warn(reason, context, "", app_err))
        }
    }
}
