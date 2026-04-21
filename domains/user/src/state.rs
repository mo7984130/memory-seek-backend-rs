use deadpool_redis::Pool;
use oss::S3Client;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::Semaphore;

use common::utils::{HashAlgorithm, TokenCipher};
use common::constants::get_password_verify_max_concurrency;

pub struct UserState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub token_cipher: Arc<TokenCipher>,
    pub s3_client: Arc<S3Client>,
    pub password_verify_semaphore: Arc<Semaphore>,
    pub hasher: HashAlgorithm,
}

impl UserState {
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        s3_client: Arc<S3Client>,
        token_cipher: Arc<TokenCipher>,
    ) -> Self {
        let max_concurrency = get_password_verify_max_concurrency();
        let hasher = common::constants::HASHER;

        Self {
            db,
            redis,
            token_cipher,
            s3_client,
            password_verify_semaphore: Arc::new(Semaphore::new(max_concurrency)),
            hasher,
        }
    }
}
