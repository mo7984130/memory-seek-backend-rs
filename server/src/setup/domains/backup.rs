use common::Result;
use common::{error::AppError, ext::ContextResultExt};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::info;

pub use backup::BackupConfig as Config;

pub struct BackupRuntime {
    pub state: Arc<backup::BackupState>,
    pub scheduler: Arc<backup::BackupScheduler>,
}

/// 初始化备份调度器
///
/// 创建 BackupState 并启动 BackupScheduler。
pub async fn init(
    db: &DatabaseConnection,
    s3_client: &Arc<oss::S3Client>,
    cfg: &Config,
) -> Result<BackupRuntime> {
    info!("初始化备份调度器");
    let bs = Arc::new(backup::BackupState::new(
        db.clone(),
        s3_client.clone(),
        cfg.clone(),
    ));
    let scheduler = backup::BackupScheduler::new(bs.clone())
        .await
        .context_error(
            "backup_init_err",
            "备份调度器初始化失败",
            AppError::InternalServerError,
        )?;
    scheduler.start().await.context_error(
        "backup_start_err",
        "备份调度器启动失败",
        AppError::InternalServerError,
    )?;
    info!("备份调度器初始化成功");

    Ok(BackupRuntime {
        state: bs,
        scheduler: Arc::new(scheduler),
    })
}
