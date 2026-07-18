use common::error::AppError;
use common::ext::ResultErrExt;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::info;

pub use backup::BackupConfig as Config;

/// 初始化备份调度器
///
/// 验证配置，创建 BackupState，启动 BackupScheduler。
/// 返回 `None` 表示未启用备份。
pub async fn init(
    db: &DatabaseConnection,
    s3_client: &Arc<oss::S3Client>,
    cfg: &Config,
) -> Result<Option<Arc<backup::BackupScheduler>>, AppError> {
    if !cfg.scheduled.enabled {
        return Ok(None);
    }

    info!("初始化备份调度器");
    let bs = Arc::new(backup::BackupState::new(
        db.clone(),
        s3_client.clone(),
        cfg.clone(),
    ));
    let scheduler = backup::BackupScheduler::new(bs.clone())
        .await
        .trace_internal_err("backup_init_err", "备份调度器初始化失败")?;
    scheduler
        .start()
        .await
        .trace_internal_err("backup_start_err", "备份调度器启动失败")?;
    info!("备份调度器初始化成功");

    Ok(Some(Arc::new(scheduler)))
}
