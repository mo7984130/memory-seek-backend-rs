use std::path::PathBuf;
use std::sync::Arc;

use oss::S3Client;

/// Storage abstraction for backup files (local filesystem + S3/OSS)
pub struct BackupStorage {
    pub local_path: PathBuf,
    pub s3_client: Arc<S3Client>,
    pub s3_prefix: String,
}

impl BackupStorage {
    pub fn new(local_path: PathBuf, s3_client: Arc<S3Client>, s3_prefix: String) -> Self {
        Self {
            local_path,
            s3_client,
            s3_prefix,
        }
    }
}
