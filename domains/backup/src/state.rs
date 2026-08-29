use sea_orm::DatabaseConnection;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::BackupConfig;
use crate::storage::BackupStorage;
use oss::S3Client;

/// 备份服务状态
pub struct BackupState {
    pub db: DatabaseConnection,
    pub storage: BackupStorage,
    pub config: BackupConfig,
    pub temp_dir: PathBuf,
    pub operation_lock: Mutex<()>,
}

impl BackupState {
    /// 创建备份领域状态并初始化存储组件.
    pub fn new(db: DatabaseConnection, s3_client: Arc<S3Client>, config: BackupConfig) -> Self {
        let s3_prefix = config.s3_prefix.trim().trim_end_matches("/");

        let temp_dir = std::env::temp_dir().join("memory-seek-backup-tmp");
        let storage = BackupStorage::new(
            PathBuf::from(&config.local_path),
            s3_client,
            s3_prefix.to_string(),
        );

        Self {
            db,
            storage,
            config,
            temp_dir,
            operation_lock: Mutex::new(()),
        }
    }

    /// 确保临时目录存在
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.temp_dir)?;
        std::fs::create_dir_all(&self.config.local_path)?;
        Ok(())
    }
}
