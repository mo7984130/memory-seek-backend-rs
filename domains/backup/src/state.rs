use std::sync::Arc;

use sea_orm::DatabaseConnection;

use crate::config::BackupConfig;

/// Shared state for the backup module
#[derive(Clone)]
pub struct BackupState {
    pub db: DatabaseConnection,
    pub config: BackupConfig,
    pub oss_client: Arc<oss::S3Client>,
}

impl BackupState {
    pub fn new(
        db: DatabaseConnection,
        config: BackupConfig,
        oss_client: Arc<oss::S3Client>,
    ) -> Self {
        Self {
            db,
            config,
            oss_client,
        }
    }
}
