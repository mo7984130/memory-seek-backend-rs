use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;

#[cfg(any(feature = "s3", feature = "backup", feature = "face-engine"))]
use std::sync::Arc;

#[cfg(feature = "email")]
use email::EmailClient;

#[cfg(feature = "face-engine")]
use std::sync::Mutex;

#[cfg(feature = "s3")]
use oss::S3Client;

#[cfg(feature = "backup")]
use backup::BackupScheduler;

// ============ Bases ============
pub struct AppBases {
    pub db: DatabaseConnection,
    pub redis: Pool,
}

// ============ Libs ============
pub struct AppLibs {
    #[cfg(feature = "email")]
    pub email_client: EmailClient,

    #[cfg(feature = "s3")]
    pub s3_client: Arc<S3Client>,

    #[cfg(feature = "face-engine")]
    pub face_engine: Arc<Mutex<insight_face_rs::FaceEngine>>,
}

// ============ AppState ============
pub struct AppState {
    pub db: DatabaseConnection,
    pub redis: Pool,

    #[cfg(feature = "email")]
    pub email_client: EmailClient,

    #[cfg(feature = "s3")]
    pub s3_client: Arc<S3Client>,

    #[cfg(feature = "backup")]
    pub backup_scheduler: Arc<BackupScheduler>,

    #[cfg(feature = "face-engine")]
    pub face_engine: Arc<Mutex<insight_face_rs::FaceEngine>>,
}
