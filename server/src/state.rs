use common::Pool;
use sea_orm::DatabaseConnection;

#[cfg(any(feature = "s3", feature = "backup", feature = "face-engine"))]
use std::sync::Arc;

#[cfg(feature = "email")]
use email::EmailClient;

#[cfg(feature = "s3")]
use oss::S3Client;

#[cfg(feature = "backup")]
use backup::{BackupScheduler, BackupState};

#[cfg(feature = "metrics")]
use metrics_exporter_prometheus::PrometheusHandle;

// ============ Bases ============
pub struct AppBases {
    pub db: DatabaseConnection,
    pub redis: Pool,

    #[cfg(feature = "metrics")]
    pub metrics_handle: PrometheusHandle,
}

// ============ Libs ============
pub struct AppLibs {
    #[cfg(feature = "email")]
    pub email_client: EmailClient,

    #[cfg(feature = "s3")]
    pub s3_client: Arc<S3Client>,

    #[cfg(feature = "face-engine")]
    pub face_engine: Arc<insight_face_rs::FaceEngine>,
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

    #[cfg(feature = "backup")]
    pub backup_state: Arc<BackupState>,

    #[cfg(feature = "metrics")]
    pub metrics_handle: PrometheusHandle,

    #[cfg(feature = "face-engine")]
    pub face_engine: Arc<insight_face_rs::FaceEngine>,
}
