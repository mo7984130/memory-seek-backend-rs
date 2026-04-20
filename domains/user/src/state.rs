use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::Semaphore;

use img_url_generator::EncryptionKey;
use common::utils::HashAlgorithm;
use common::constants::get_password_verify_max_concurrency;

pub struct UserState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub encryption_key: EncryptionKey,
    pub password_verify_semaphore: Arc<Semaphore>,
    pub hasher: HashAlgorithm,
}

impl UserState {
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        encryption_key: EncryptionKey,
    ) -> Self {
        let max_concurrency = get_password_verify_max_concurrency();
        let hasher = common::constants::HASHER;

        Self {
            db,
            redis,
            encryption_key,
            password_verify_semaphore: Arc::new(Semaphore::new(max_concurrency)),
            hasher,
        }
    }
}
