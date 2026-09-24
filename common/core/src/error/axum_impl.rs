//! `AppError` 的 axum 响应适配。
//!
//! ⚠️ 受**孤儿规则**约束(`IntoResponse` 是外部 trait、`AppError` 是本地类型),
//! 这些实现必须与 `AppError` 同 crate,因此随错误类型一起放在 `common-core`,
//! 并由 `axum` feature 门控。

use axum::{http::StatusCode, response::IntoResponse};

use crate::{error::AppError, r::R};

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Ignore => StatusCode::OK,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        R::err(self.status_code().as_u16(), self.to_string().as_str()).into_response()
    }
}
