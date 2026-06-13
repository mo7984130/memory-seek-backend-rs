use common::utils::TokenCipher;
use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[cfg(feature = "s3")]
use oss::S3Client;

// ============ Bases ============
pub struct AppBases {
    pub db: DatabaseConnection,
    pub redis: Pool,
}

// ============ Libs ============
pub struct AppLibs {
    pub token_cipher: Arc<TokenCipher>,

    #[cfg(feature = "s3")]
    pub s3_client: Arc<S3Client>,
}

// ============ AppState ============
pub struct AppState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub token_cipher: Arc<TokenCipher>,

    #[cfg(feature = "s3")]
    pub s3_client: Arc<S3Client>,
}

impl From<(AppBases, AppLibs)> for AppState {
    fn from((bases, libs): (AppBases, AppLibs)) -> Self {
        Self {
            db: bases.db,
            redis: bases.redis,
            token_cipher: libs.token_cipher,
            #[cfg(feature = "s3")]
            s3_client: libs.s3_client,
        }
    }
}
