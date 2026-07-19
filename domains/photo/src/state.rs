use std::sync::Arc;
#[cfg(feature = "face")]
use std::sync::Mutex;

use common::utils::TokenCipher;
use deadpool_redis::Pool;
use oss::S3Client;
use sea_orm::DatabaseConnection;

#[cfg(feature = "face")]
use backup::storage::BackupStorage;

pub struct PhotoState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub s3_client: Arc<S3Client>,
    pub token_cipher: Arc<TokenCipher>,
    #[cfg(feature = "face")]
    pub face_engine: Arc<Mutex<insight_face_rs::FaceEngine>>,
    #[cfg(feature = "face")]
    pub backup_storage: Option<BackupStorage>,
}

impl PhotoState {
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        s3_client: Arc<S3Client>,
        token_cipher: Arc<TokenCipher>,
        #[cfg(feature = "face")] face_engine: Arc<Mutex<insight_face_rs::FaceEngine>>,
        #[cfg(feature = "face")] backup_storage: Option<BackupStorage>,
    ) -> Self {
        Self {
            db,
            redis,
            s3_client,
            token_cipher,
            #[cfg(feature = "face")]
            face_engine,
            #[cfg(feature = "face")]
            backup_storage,
        }
    }
}
