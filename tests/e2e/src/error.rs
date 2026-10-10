//! e2e 场景统一错误类型:桥接 api-rs 的 [`ApiError`] 与框架/基础设施的 [`HttpError`]。
//!
//! 框架按 [`ScenarioError::kind`] 对错误分类、按 `is_timeout` 区分软失败(超时)。
//! 此处把两条来源的错误归一到同一套分类语义(如 `http_status_{code}`),保证报告与
//! 退避行为与迁移前一致。

use std::borrow::Cow;

use memory_seek_api::{ApiError, TransportError};
use memseek_test::ctxlibs::http_client::{HttpError, reqwest};
use memseek_test::error::ScenarioError;

/// e2e 场景错误。
#[derive(Debug, thiserror::Error)]
pub enum E2eError {
    /// 产品 API 调用错误(api-rs)。
    #[error(transparent)]
    Api(#[from] ApiError),
    /// 基础设施错误(MailHog 等经框架 client)。
    #[error(transparent)]
    Infra(#[from] HttpError),
    /// 场景内部的非 HTTP 失败(如直连 DB/Redis 的收尾失败)。
    #[error("{0}")]
    Internal(String),
}

impl E2eError {
    /// 构造内部失败。
    pub fn internal(reason: impl Into<String>) -> Self {
        Self::Internal(reason.into())
    }
}

impl ScenarioError for E2eError {
    fn kind(&self) -> Cow<'static, str> {
        match self {
            Self::Api(error) => api_kind(error),
            Self::Infra(error) => error.kind(),
            Self::Internal(_) => "internal".into(),
        }
    }

    fn is_timeout(&self) -> bool {
        match self {
            Self::Api(ApiError::Transport(error)) => is_transport_timeout(error),
            Self::Infra(error) => error.is_timeout(),
            Self::Api(_) | Self::Internal(_) => false,
        }
    }
}

/// `ApiError` → 错误分类(与 [`HttpError`] 的分类语义对齐)。
fn api_kind(error: &ApiError) -> Cow<'static, str> {
    match error {
        ApiError::HttpStatus { code, .. } => format!("http_status_{code}").into(),
        ApiError::Decode(_) => "decode".into(),
        ApiError::NotLoggedIn => "not_logged_in".into(),
        ApiError::TokenExpired => "token_expired".into(),
        ApiError::Transport(error) => transport_kind(error),
    }
}

/// 传输层错误分类:尽力下钻到 `reqwest::Error`,否则归为 `transport`。
fn transport_kind(error: &TransportError) -> Cow<'static, str> {
    let Some(error) = error.downcast_ref::<reqwest::Error>() else {
        return "transport".into();
    };
    if error.is_timeout() {
        "timeout".into()
    } else if error.is_connect() {
        "connect".into()
    } else if error.is_body() || error.is_decode() {
        "decode".into()
    } else if error.is_request() || error.is_builder() {
        "request".into()
    } else {
        "reqwest".into()
    }
}

/// 传输层错误是否为超时。
fn is_transport_timeout(error: &TransportError) -> bool {
    error
        .downcast_ref::<reqwest::Error>()
        .is_some_and(reqwest::Error::is_timeout)
}

/// 把 [`ApiError`] 适配为框架的 [`HttpError`]。
///
/// 供仍需返回 `HttpError` 的共享辅助函数(如 `scenarios::user::session::login`,
/// visual/system 等未迁移场景也用它)使用;错误分类语义保持不变。
pub fn api_to_http(error: ApiError) -> HttpError {
    use reqwest::{Method, StatusCode};

    fn status_error(status: StatusCode, detail: String) -> HttpError {
        HttpError::Status {
            status,
            method: Method::GET,
            url: String::new(),
            request_body: None,
            response_body: Some(Ok(detail)),
        }
    }

    match error {
        ApiError::HttpStatus { code, msg } => status_error(
            StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            msg,
        ),
        ApiError::Decode(error) => HttpError::Serde(error),
        ApiError::Transport(error) => match error.downcast::<reqwest::Error>() {
            Ok(error) => HttpError::Reqwest(*error),
            Err(error) => status_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        },
        ApiError::NotLoggedIn => status_error(StatusCode::UNAUTHORIZED, "未登录".to_owned()),
        ApiError::TokenExpired => status_error(StatusCode::UNAUTHORIZED, "登录已失效".to_owned()),
    }
}

/// 把 api 结果压成 HTTP 状态码:成功归一为 `200`,业务失败取 `HttpStatus.code`;
/// 其余错误(传输/解析/未登录等)原样上抛。
///
/// 供“期望某个错误码”的负向场景使用(如断言登录失败返回 400)。
pub fn status<T>(result: Result<T, ApiError>) -> Result<u16, E2eError> {
    match result {
        Ok(_) => Ok(200),
        Err(ApiError::HttpStatus { code, .. }) => Ok(code),
        Err(error) => Err(E2eError::Api(error)),
    }
}
