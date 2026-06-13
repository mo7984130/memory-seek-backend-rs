use crate::r::R;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use std::borrow::Cow;
use thiserror::Error;

/// 应用层统一错误类型
///
/// 封装所有业务错误场景，自动实现 `IntoResponse`，可直接作为 axum handler 返回值。
/// 通过 `#[error]` 宏自动生成 `Display` 实现，配合 `R::err` 输出统一 JSON 响应。
#[derive(Debug, Error)]
pub enum AppError {
    #[error("认证失败")]
    Unauthorized,

    #[error("{0}")]
    BadRequest(Cow<'static, str>),

    #[error("{0}")]
    NotFound(Cow<'static, str>),

    #[error("{0}")]
    Forbidden(Cow<'static, str>),

    #[error("{0}")]
    Conflict(Cow<'static, str>),

    #[error("服务器内部错误")]
    InternalServerError,

    #[error("忽略的错误, 不应该输出")]
    Ignore,
}

impl AppError {
    /// 获取错误对应的 HTTP 状态码
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Ignore => StatusCode::OK,
        }
    }

    /// 创建请求参数错误
    ///
    /// # 参数
    /// - `msg`: 错误描述信息，支持 `&str` 或 `String`
    pub fn bad_request(msg: impl Into<Cow<'static, str>>) -> Self {
        Self::BadRequest(msg.into())
    }

    /// 创建资源不存在错误
    ///
    /// # 参数
    /// - `msg`: 错误描述信息，支持 `&str` 或 `String`
    pub fn not_found(msg: impl Into<Cow<'static, str>>) -> Self {
        Self::NotFound(msg.into())
    }

    /// 创建权限不足错误
    ///
    /// # 参数
    /// - `msg`: 错误描述信息，支持 `&str` 或 `String`
    pub fn forbidden(msg: impl Into<Cow<'static, str>>) -> Self {
        Self::Forbidden(msg.into())
    }

    /// 创建冲突错误
    ///
    /// # 参数
    /// - `msg`: 错误描述信息，支持 `&str` 或 `String`
    pub fn conflict(msg: impl Into<Cow<'static, str>>) -> Self {
        Self::Conflict(msg.into())
    }
}

/// 将 `AppError` 转换为 HTTP 响应
///
/// 使用 `R::err` 构建统一 JSON 格式的错误响应体，HTTP 状态码通过 `status_code()` 获取。
impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        R::err(self.status_code().as_u16(), self.to_string().as_str()).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- status_code tests ---

    #[test]
    fn status_code_unauthorized() {
        assert_eq!(
            AppError::Unauthorized.status_code(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn status_code_bad_request() {
        assert_eq!(
            AppError::BadRequest("msg".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn status_code_not_found() {
        assert_eq!(
            AppError::NotFound("msg".into()).status_code(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn status_code_forbidden() {
        assert_eq!(
            AppError::Forbidden("msg".into()).status_code(),
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn status_code_conflict() {
        assert_eq!(
            AppError::Conflict("msg".into()).status_code(),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn status_code_internal_server_error() {
        assert_eq!(
            AppError::InternalServerError.status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn status_code_ignore() {
        assert_eq!(AppError::Ignore.status_code(), StatusCode::OK);
    }

    // --- constructor tests ---

    #[test]
    fn bad_request_constructor() {
        let err = AppError::bad_request("invalid input");
        match err {
            AppError::BadRequest(msg) => assert_eq!(msg, "invalid input"),
            _ => panic!("expected BadRequest"),
        }
    }

    #[test]
    fn bad_request_constructor_with_string() {
        let err = AppError::bad_request(String::from("owned string"));
        match err {
            AppError::BadRequest(msg) => assert_eq!(msg, "owned string"),
            _ => panic!("expected BadRequest"),
        }
    }

    #[test]
    fn not_found_constructor() {
        let err = AppError::not_found("missing resource");
        match err {
            AppError::NotFound(msg) => assert_eq!(msg, "missing resource"),
            _ => panic!("expected NotFound"),
        }
    }

    #[test]
    fn conflict_constructor() {
        let err = AppError::conflict("already exists");
        match err {
            AppError::Conflict(msg) => assert_eq!(msg, "already exists"),
            _ => panic!("expected Conflict"),
        }
    }

    #[test]
    fn forbidden_constructor() {
        let err = AppError::forbidden("no access");
        match err {
            AppError::Forbidden(msg) => assert_eq!(msg, "no access"),
            _ => panic!("expected Forbidden"),
        }
    }

    #[test]
    fn unauthorized_is_variant() {
        let err = AppError::Unauthorized;
        assert!(matches!(err, AppError::Unauthorized));
    }

    #[test]
    fn internal_server_error_is_variant() {
        let err = AppError::InternalServerError;
        assert!(matches!(err, AppError::InternalServerError));
    }

    #[test]
    fn ignore_is_variant() {
        let err = AppError::Ignore;
        assert!(matches!(err, AppError::Ignore));
    }

    // --- Display (thiserror) tests ---

    #[test]
    fn display_unauthorized() {
        assert_eq!(AppError::Unauthorized.to_string(), "认证失败");
    }

    #[test]
    fn display_bad_request() {
        assert_eq!(AppError::BadRequest("msg".into()).to_string(), "msg");
    }

    #[test]
    fn display_internal_server_error() {
        assert_eq!(AppError::InternalServerError.to_string(), "服务器内部错误");
    }

    #[test]
    fn display_ignore() {
        assert_eq!(AppError::Ignore.to_string(), "忽略的错误, 不应该输出");
    }
}
