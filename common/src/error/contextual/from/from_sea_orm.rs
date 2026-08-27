use crate::error::{AppError, ContextualError};

impl From<sea_orm::DbErr> for ContextualError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::error("db_err", "数据库错误", error, AppError::InternalServerError)
    }
}
