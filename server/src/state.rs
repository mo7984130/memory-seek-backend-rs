use common::utils::TokenCipher;
use deadpool_redis::Pool;
use email::EmailClient;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
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
    #[allow(dead_code)]
    pub _log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

// ============ Libs ============
pub struct AppLibs {
    pub token_cipher: Arc<TokenCipher>,
    pub email_client: EmailClient,

    #[cfg(feature = "s3")]
    pub s3_client: Arc<S3Client>,

    #[cfg(feature = "face-engine")]
    pub face_engine: Arc<Mutex<insight_face_rs::FaceEngine>>,
}

// ============ AppState ============
pub struct AppState {
    #[allow(dead_code)]
    pub db: DatabaseConnection,
    pub redis: Pool,
    #[allow(dead_code)]
    pub token_cipher: Arc<TokenCipher>,
    #[allow(dead_code)]
    pub email_client: EmailClient,

    #[cfg(feature = "s3")]
    pub s3_client: Arc<S3Client>,

    #[cfg(feature = "backup")]
    pub backup_scheduler: Option<Arc<BackupScheduler>>,

    #[cfg(feature = "face-engine")]
    pub face_engine: Arc<Mutex<insight_face_rs::FaceEngine>>,
}
