use crate::{error::AppError, ext::log_err_with_err};
use sea_orm::DbErr;

impl From<DbErr> for AppError {
    #[track_caller]
    fn from(value: DbErr) -> Self {
        log_err_with_err(
            "db_err",
            "数据库错误",
            &value,
            AppError::InternalServerError,
        )
    }
}
