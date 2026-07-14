use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::BackupConfig;
use crate::storage::BackupStorage;
use oss::S3Client;

/// 备份服务状态
pub struct BackupState {
    pub db: DatabaseConnection,
    pub storage: BackupStorage,
    pub config: BackupConfig,
    pub temp_dir: PathBuf,
    pub last_hashes: RwLock<HashMap<String, String>>,
}

impl BackupState {
    pub fn new(db: DatabaseConnection, s3_client: Arc<S3Client>, config: BackupConfig) -> Self {
        let temp_dir = PathBuf::from(&config.local_path).join(".tmp");
        let storage = BackupStorage::new(
            PathBuf::from(&config.local_path),
            s3_client,
            config.s3_prefix.clone(),
        );

        Self {
            db,
            storage,
            config,
            temp_dir,
            last_hashes: RwLock::new(HashMap::new()),
        }
    }

    /// 确保临时目录存在
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.temp_dir)?;
        std::fs::create_dir_all(&self.config.local_path)?;
        Ok(())
    }
}
