use std::sync::Arc;

use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;
use common::utils::TokenCipher;
use oss::S3Client;

#[cfg(feature = "face_recognition")]
use tokio::sync::mpsc;

pub struct PhotoState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub s3_client: Arc<S3Client>,
    #[cfg(feature = "face_recognition")]
    pub face_tx: mpsc::Sender<crate::FaceTask>,
    pub token_cipher: Arc<TokenCipher>,
}

impl PhotoState {
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        s3_client: Arc<S3Client>,
        #[cfg(feature =  "face_recognition")]
        face_tx: mpsc::Sender<crate::FaceTask>,
        token_cipher: Arc<TokenCipher>,
    ) -> Self {
        Self {
            db,
            redis,
            s3_client,
            #[cfg(feature =  "face_recognition")]
            face_tx,
            token_cipher,
        }
    }
}
