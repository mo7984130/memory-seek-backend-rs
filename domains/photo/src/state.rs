use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;

use img_url_generator::EncryptionKey;
use oss::S3Client;

#[cfg(feature = "face_recognition")]
use crate::services::photo_service::FaceTask;

#[cfg(feature = "face_recognition")]
use tokio::sync::mpsc;

pub struct AppState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub s3_client: S3Client,
    #[cfg(feature = "face_recognition")]
    pub face_tx: Option<mpsc::Sender<FaceTask>>,
    pub encryption_key: EncryptionKey,
}
