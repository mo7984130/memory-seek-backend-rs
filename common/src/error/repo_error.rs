use std::borrow::Cow;

use crate::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("{0}")]
    Conflict(Cow<'static, str>),
    #[error("{0}")]
    NotFound(Cow<'static, str>),
    #[error("数据库内部错误")]
    Internal,
}

impl From<RepoError> for AppError {
    fn from(value: RepoError) -> Self {
        match value {
            RepoError::Conflict(msg) => AppError::Conflict(msg),
            RepoError::NotFound(msg) => AppError::NotFound(msg),
            RepoError::Internal => AppError::InternalServerError,
        }
    }
}
