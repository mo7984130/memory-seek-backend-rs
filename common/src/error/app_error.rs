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

    #[error("请求体过大")]
    PayloadTooLarge,
}

impl AppError {
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
