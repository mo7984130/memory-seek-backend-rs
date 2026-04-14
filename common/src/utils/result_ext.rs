use crate::error::AppError;
use crate::r::R;
use std::fmt::{Debug, Display};
use tracing::{error, warn};

pub trait ResultExt<T, E> {
    fn map_internal_err(self, context: &'static str) -> Result<T, AppError>;
    fn map_bad_request_err(self, context: &'static str) -> Result<T, AppError>;

    fn to_bad_request_error(self) -> Result<T, AppError>
    where
        E: Display;

    fn into_ok_res(self) -> Result<R<T>, AppError>
    where
        Self: Sized,
        E: Into<AppError>,
        T: serde::Serialize;

    fn trace_internal_err(self, reason: &'static str, context: &'static str) -> Result<T, AppError>
    where
        E: Debug;

    fn trace_bad_request_err(self, reason: &'static str, context: &'static str) -> Result<T, AppError>
    where
        E: Display;
}

impl<T, E: Debug> ResultExt<T, E> for Result<T, E> {
    #[inline]
    fn map_internal_err(self, context: &'static str) -> Result<T, AppError> {
        self.map_err(|e| {
            error!("内部错误: {} \n {:?}", context, e);
            AppError::InternalServerError
        })
    }

    #[inline]
    fn map_bad_request_err(self, context: &'static str) -> Result<T, AppError> {
        self.map_err(|_| {
            AppError::bad_request(context)
        })
    }

    /// TODO static str
    #[inline]
    fn to_bad_request_error(self) -> Result<T, AppError>
    where
        E: Display
    {
        self.map_err(|e| {
            AppError::BadRequest(e.to_string().into())
        })
    }

    #[inline]
    fn into_ok_res(self) -> Result<R<T>, AppError>
    where
        E: Into<AppError>,
        T: serde::Serialize,
    {
        self.map(R::ok).map_err(|e| e.into())
    }

    fn trace_internal_err(self, reason: &'static str, context: &'static str) -> Result<T, AppError>
    where
        E: Debug
    {
        self.map_err(|e| {
            error!(%reason, status = "error", error = ?e, "{context}");
            AppError::InternalServerError
        })
    }

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

pub trait ToOkExt {
    fn into_ok<E>(self) -> Result<Self, E>
    where
        Self: Sized;

    // 针对你常用的 AppError 进一步简化
    fn ok_res(self) -> Result<Self, AppError>
    where
        Self: Sized;
}

impl<T> ToOkExt for T {
    #[inline]
    fn into_ok<E>(self) -> Result<Self, E> {
        Ok(self)
    }

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