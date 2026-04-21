use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;
use common::utils::TokenCipher;
use oss::S3Client;

pub struct PhotoState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub s3_client: S3Client,
    pub token_cipher: TokenCipher,
}

impl PhotoState {
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        s3_client: S3Client,
        token_cipher: TokenCipher,
    ) -> Self {
        Self {
            db,
            redis,
            s3_client,
            token_cipher,
        }
    }
}
