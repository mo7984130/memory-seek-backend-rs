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
}

/// 将静态字符串转换为 `BadRequest` 错误
///
/// 便于在 handler 中使用 `?` 运算符快速返回错误。
impl From<&'static str> for AppError {
    fn from(msg: &'static str) -> Self {
        AppError::BadRequest(msg.into())
    }
}

impl AppError {
    /// 获取错误对应的 HTTP 状态码
    ///
    /// # 返回
    /// 返回与错误类型对应的 `StatusCode`，映射关系：
    /// - `Unauthorized` -> 401
    /// - `InternalServerError` -> 500
    /// - `BadRequest` -> 400
    /// - `NotFound` -> 404
    /// - `Forbidden` -> 403
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Conflict(_) => StatusCode::CONFLICT,
        }
    }

    /// 创建请求参数错误
    ///
    /// # 参数
    /// - `msg`: 错误描述信息，支持 `&str` 或 `String`
    ///
    /// # 返回
    /// 返回 `BadRequest` 变体的 `AppError`
    pub fn bad_request<S: Into<Cow<'static, str>>>(msg: S) -> Self {
        Self::BadRequest(msg.into())
    }

    /// 创建资源不存在错误
    ///
    /// # 参数
    /// - `msg`: 错误描述信息，支持 `&str` 或 `String`
    ///
    /// # 返回
    /// 返回 `NotFound` 变体的 `AppError`
    pub fn not_found<S: Into<Cow<'static, str>>>(msg: S) -> Self {
        Self::NotFound(msg.into())
    }

    /// 创建权限不足错误
    ///
    /// # 参数
    /// - `msg`: 错误描述信息，支持 `&str` 或 `String`
    ///
    /// # 返回
    /// 返回 `Forbidden` 变体的 `AppError`
    pub fn forbidden<S: Into<Cow<'static, str>>>(msg: S) -> Self {
        Self::Forbidden(msg.into())
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
