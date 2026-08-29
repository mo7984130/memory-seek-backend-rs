use common::error::{AppError, ContextualError};
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

    /// 数据库查询失败
    #[error("备份数据库操作失败: {0}")]
    Db(#[from] sea_orm::DbErr),

    /// PostgreSQL 二进制 COPY 导出失败
    #[error("备份 PostgreSQL COPY 操作失败: {0}")]
    Copy(#[from] sqlx::Error),

    #[error("备份清单处理失败: {0}")]
    Json(#[from] serde_json::Error),

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

pub type Result<T> = std::result::Result<T, BackupError>;

impl From<BackupError> for AppError {
    #[track_caller]
    fn from(err: BackupError) -> Self {
        ContextualError::error(
            "backup_error",
            "备份执行失败",
            err,
            AppError::InternalServerError,
        )
        .emit()
    }
}
