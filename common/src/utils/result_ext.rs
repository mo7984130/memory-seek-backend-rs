use crate::error::AppError;
use crate::r::R;
use std::fmt::{Debug, Display};
use tracing::{error, warn};

/// 为 `Result<T, E>` 提供到 `AppError` 的便捷转换方法
pub trait ResultExt<T, E> {
    /// 将错误映射为 `AppError::InternalServerError`，并通过 `tracing::error!` 记录日志
    ///
    /// # 参数
    /// - `context`: 错误上下文描述，用于日志输出
    ///
    /// # 返回
    /// 成功时返回原始值，失败时返回 `AppError::InternalServerError`
    fn map_internal_err(self, context: &'static str) -> Result<T, AppError>;

    /// 将错误映射为 `AppError::BadRequest`，原始错误信息被丢弃
    ///
    /// # 参数
    /// - `context`: 错误描述，作为 `BadRequest` 的消息内容
    ///
    /// # 返回
    /// 成功时返回原始值，失败时返回 `AppError::BadRequest`
    fn map_bad_request_err(self, context: &'static str) -> Result<T, AppError>;

    /// 将错误转换为 `AppError::BadRequest`，保留原始错误的 `Display` 信息
    ///
    /// # 返回
    /// 成功时返回原始值，失败时返回 `AppError::BadRequest(e.to_string())`
    fn to_bad_request_error(self) -> Result<T, AppError>
    where
        E: Display;

    /// 将 `Result<T, E>` 转换为 `Result<R<T>, AppError>`，成功值包装为 `R::ok`
    ///
    /// # 返回
    /// 成功时返回 `R::ok(value)`，失败时将错误转换为 `AppError`
    fn into_ok_res(self) -> Result<R<T>, AppError>
    where
        Self: Sized,
        E: Into<AppError>,
        T: serde::Serialize;

    /// 将错误映射为 `AppError::InternalServerError`，通过 `tracing::error!` 记录结构化日志
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `context`: 日志中的上下文描述
    ///
    /// # 返回
    /// 成功时返回原始值，失败时返回 `AppError::InternalServerError`
    fn trace_internal_err(self, reason: &'static str, context: &'static str) -> Result<T, AppError>
    where
        E: Debug;

    /// 将错误映射为 `AppError::BadRequest`，通过 `tracing::warn!` 记录结构化日志
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `context`: 日志中的上下文描述，同时作为 `BadRequest` 的消息
    ///
    /// # 返回
    /// 成功时返回原始值，失败时返回 `AppError::BadRequest(context)`
    fn trace_bad_request_err(self, reason: &'static str, context: &'static str) -> Result<T, AppError>
    where
        E: Display;
}

impl<T, E: Debug> ResultExt<T, E> for Result<T, E> {
    /// 将错误映射为 `AppError::InternalServerError`，并通过 `tracing::error!` 记录日志
    ///
    /// # 参数
    /// - `context`: 错误上下文描述，用于日志输出
    ///
    /// # 返回
    /// 成功时返回原始值，失败时返回 `AppError::InternalServerError`
    #[inline]
    fn map_internal_err(self, context: &'static str) -> Result<T, AppError> {
        self.map_err(|e| {
            error!("内部错误: {} \n {:?}", context, e);
            AppError::InternalServerError
        })
    }

    /// 将错误映射为 `AppError::BadRequest`，原始错误信息被丢弃
    ///
    /// # 参数
    /// - `context`: 错误描述，作为 `BadRequest` 的消息内容
    ///
    /// # 返回
    /// 成功时返回原始值，失败时返回 `AppError::BadRequest`
    #[inline]
    fn map_bad_request_err(self, context: &'static str) -> Result<T, AppError> {
        self.map_err(|_| {
            AppError::bad_request(context)
        })
    }

    /// 将错误转换为 `AppError::BadRequest`，保留原始错误的 `Display` 信息
    ///
    /// # 返回
    /// 成功时返回原始值，失败时返回 `AppError::BadRequest(e.to_string())`
    #[inline]
    fn to_bad_request_error(self) -> Result<T, AppError>
    where
        E: Display
    {
        self.map_err(|e| {
            AppError::BadRequest(e.to_string().into())
        })
    }

    /// 将 `Result<T, E>` 转换为 `Result<R<T>, AppError>`，成功值包装为 `R::ok`
    ///
    /// # 返回
    /// 成功时返回 `R::ok(value)`，失败时将错误转换为 `AppError`
    #[inline]
    fn into_ok_res(self) -> Result<R<T>, AppError>
    where
        E: Into<AppError>,
        T: serde::Serialize,
    {
        self.map(R::ok).map_err(|e| e.into())
    }

    /// 将错误映射为 `AppError::InternalServerError`，通过 `tracing::error!` 记录结构化日志
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `context`: 日志中的上下文描述
    ///
    /// # 返回
    /// 成功时返回原始值，失败时返回 `AppError::InternalServerError`
    fn trace_internal_err(self, reason: &'static str, context: &'static str) -> Result<T, AppError>
    where
        E: Debug
    {
        self.map_err(|e| {
            error!(%reason, status = "error", error = ?e, "{context}");
            AppError::InternalServerError
        })
    }

    /// 将错误映射为 `AppError::BadRequest`，通过 `tracing::warn!` 记录结构化日志
    ///
    /// # 参数
    /// - `reason`: 日志中的 `reason` 字段
    /// - `context`: 日志中的上下文描述，同时作为 `BadRequest` 的消息
    ///
    /// # 返回
    /// 成功时返回原始值，失败时返回 `AppError::BadRequest(context)`
    fn trace_bad_request_err(self, reason: &'static str, context: &'static str) -> Result<T, AppError>
    where
        E: Display
    {
        self.map_err(|e| {
            warn!(%reason, status="failed", error = %e, "{context}");
            AppError::bad_request(context)
        })
    }
}

/// 为任意类型提供便捷的 `Ok` 包装方法
pub trait ToOkExt {
    /// 将值包装为 `Ok(self)`
    ///
    /// # 返回
    /// 返回 `Ok(self)`，错误类型由调用方推断
    fn into_ok<E>(self) -> Result<Self, E>
    where
        Self: Sized;

    /// 将值包装为 `Ok(self)`，错误类型固定为 `AppError`
    ///
    /// # 返回
    /// 返回 `Result<Self, AppError>` 的 `Ok` 变体
    fn ok_res(self) -> Result<Self, AppError>
    where
        Self: Sized;
}

impl<T> ToOkExt for T {
    /// 将值包装为 `Ok(self)`
    ///
    /// # 返回
    /// 返回 `Ok(self)`
    #[inline]
    fn into_ok<E>(self) -> Result<Self, E> {
        Ok(self)
    }

    /// 将值包装为 `Ok(self)`，错误类型固定为 `AppError`
    ///
    /// # 返回
    /// 返回 `Result<Self, AppError>` 的 `Ok` 变体
    #[inline]
    fn ok_res(self) -> Result<Self, AppError> {
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 map_internal_err 在 Ok 情况下的处理
    #[test]
    fn test_map_internal_err_with_ok() {
        let result: Result<i32, &str> = Ok(42);
        let mapped = result.map_internal_err("test error");
        assert!(mapped.is_ok());
        assert_eq!(mapped.unwrap(), 42);
    }

    /// 测试 map_internal_err 在 Err 情况下转换为 InternalServerError
    #[test]
    fn test_map_internal_err_with_error() {
        let result: Result<i32, &str> = Err("test");
        let mapped = result.map_internal_err("test error");
        assert!(mapped.is_err());
        assert!(matches!(mapped.unwrap_err(), AppError::InternalServerError));
    }

    /// 测试 map_bad_request_err 在 Ok 情况下的处理
    #[test]
    fn test_map_bad_request_err_with_ok() {
        let result: Result<i32, &str> = Ok(42);
        let mapped = result.map_bad_request_err("test error");
        assert!(mapped.is_ok());
        assert_eq!(mapped.unwrap(), 42);
    }

    /// 测试 map_bad_request_err 在 Err 情况下转换为 BadRequest
    #[test]
    fn test_map_bad_request_err_with_error() {
        let result: Result<i32, &str> = Err("test");
        let mapped = result.map_bad_request_err("test error");
        assert!(mapped.is_err());
        assert!(matches!(mapped.unwrap_err(), AppError::BadRequest(_)));
    }

    /// 测试 to_bad_request_error 在 Ok 情况下的处理
    #[test]
    fn test_to_bad_request_error_with_ok() {
        let result: Result<i32, &str> = Ok(42);
        let mapped = result.to_bad_request_error();
        assert!(mapped.is_ok());
        assert_eq!(mapped.unwrap(), 42);
    }

    /// 测试 to_bad_request_error 在 Err 情况下保留错误信息并转换为 BadRequest
    #[test]
    fn test_to_bad_request_error_with_error() {
        let result: Result<i32, &str> = Err("custom error");
        let mapped = result.to_bad_request_error();
        assert!(mapped.is_err());
        if let AppError::BadRequest(msg) = mapped.unwrap_err() {
            assert_eq!(msg, "custom error");
        } else {
            panic!("Expected BadRequest error");
        }
    }

    /// 测试 into_ok_res 在 Ok 情况下包装为 R 结构体
    #[test]
    fn test_into_ok_res_with_ok() {
        let result: Result<i32, &str> = Ok(42);
        let mapped = result.into_ok_res();
        assert!(mapped.is_ok());
        let r = mapped.unwrap();
        assert_eq!(r.data, Some(42));
    }

    /// 测试 into_ok_res 在 Err 情况下的错误转换
    #[test]
    fn test_into_ok_res_with_error() {
        let result: Result<i32, AppError> = Err(AppError::bad_request("test"));
        let mapped = result.into_ok_res();
        assert!(mapped.is_err());
        assert!(matches!(mapped.unwrap_err(), AppError::BadRequest(_)));
    }

    /// 测试 into_ok 扩展方法将值包装为 Ok
    #[test]
    fn test_into_ok() {
        let value = 42;
        let result: Result<i32, &str> = value.into_ok();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    /// 测试 ok_res 扩展方法将值包装为 Ok 并指定 AppError 错误类型
    #[test]
    fn test_ok_res() {
        let value = 42;
        let result: Result<i32, AppError> = value.ok_res();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
}