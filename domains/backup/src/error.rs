use common::error::AppError;
use common::ext::log_err_with_source;
use thiserror::Error;

/// 备份领域统一错误类型
///
/// 领域内部函数返回该错误，边界处通过 [`From<BackupError> for AppError`] 转换，
/// 统一走 `common` 的结构化错误日志（reason / status / error / caller 等字段）。
#[derive(Debug, Error)]
pub enum BackupError {
    /// 本地文件系统操作失败（建目录、复制、读取、删除等）
    #[error("备份文件操作失败: {0}")]
    Io(#[from] std::io::Error),

    /// CSV 写入/读取失败
    #[error("CSV 导出失败: {0}")]
    Csv(#[from] csv::Error),

    /// 数据库查询失败
    #[error("备份数据库操作失败: {0}")]
    Db(#[from] sea_orm::DbErr),

    /// S3 上传/删除失败
    #[error("备份 S3 存储操作失败: {0}")]
    S3(#[from] oss::OssError),

    /// 备份调度器（cron）失败
    #[error("备份调度器操作失败: {0}")]
    Scheduler(#[from] tokio_cron_scheduler::JobSchedulerError),

    /// 业务校验类错误（如目标表不存在）
    #[error("{0}")]
    Msg(String),
}

impl From<BackupError> for AppError {
    #[track_caller]
    fn from(err: BackupError) -> Self {
        match err {
            BackupError::Io(e) => log_err_with_source(
                "backup_io_error",
                "备份文件操作失败",
                e,
                AppError::InternalServerError,
            ),
            BackupError::Csv(e) => log_err_with_source(
                "backup_csv_error",
                "CSV 导出失败",
                e,
                AppError::InternalServerError,
            ),
            BackupError::Db(e) => log_err_with_source(
                "backup_db_error",
                "备份数据库操作失败",
                e,
                AppError::InternalServerError,
            ),
            BackupError::S3(e) => log_err_with_source(
                "backup_s3_error",
                "备份 S3 存储操作失败",
                e,
                AppError::InternalServerError,
            ),
            BackupError::Scheduler(e) => log_err_with_source(
                "backup_scheduler_error",
                "备份调度器操作失败",
                e,
                AppError::InternalServerError,
            ),
            BackupError::Msg(msg) => log_err_with_source(
                "backup_error",
                "备份执行失败",
                msg,
                AppError::InternalServerError,
            ),
        }
    }
}
