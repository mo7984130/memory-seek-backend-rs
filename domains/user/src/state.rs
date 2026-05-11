use deadpool_redis::Pool;
use oss::S3Client;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use common::utils::TokenCipher;

pub struct UserState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub token_cipher: Arc<TokenCipher>,
    pub s3_client: Arc<S3Client>,
}

impl UserState {
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        s3_client: Arc<S3Client>,
        token_cipher: Arc<TokenCipher>,
    ) -> Self {
        Self {
            db,
            redis,
            token_cipher,
            s3_client,
        }
    }
}
