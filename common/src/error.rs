use crate::r::R;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use std::borrow::Cow;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("认证失败")]
    Unauthorized,

    #[error("{0}")]
    BadRequest(Cow<'static, str>),

    #[error("服务器内部错误")]
    InternalServerError
}

impl From<&'static str> for AppError {
    fn from(msg: &'static str) -> Self {
        AppError::BadRequest(msg.into())
    }
}

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
        }
    }

    pub fn bad_request<S: Into<Cow<'static, str>>>(msg: S) -> Self {
        Self::BadRequest(msg.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        R::err(self.status_code().as_u16(), self.to_string().as_str()).into_response()
    }
}