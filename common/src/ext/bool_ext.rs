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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn true_ok_or_warn_returns_ok() {
        let result = true.ok_or_warn("test_reason", "test_context", AppError::BadRequest("bad".into()));
        assert!(result.is_ok());
    }

    #[test]
    fn false_ok_or_warn_returns_err() {
        let result = false.ok_or_warn("test_reason", "test_context", AppError::BadRequest("bad".into()));
        assert!(result.is_err());
    }
}
