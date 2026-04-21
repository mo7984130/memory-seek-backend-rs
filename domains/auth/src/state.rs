use deadpool_redis::Pool;
use email::EmailClient;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::Semaphore;

use common::utils::{HashAlgorithm, TokenCipher};
use common::constants::get_password_verify_max_concurrency;

/// 认证服务状态
pub struct AuthState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub email_client: EmailClient,
    pub token_cipher: Arc<TokenCipher>,
    /// 密码验证并发信号量
    /// 用于限制同时进行的密码验证数量，防止 CPU 密集型操作抢占 runtime 资源
    pub password_verify_semaphore: Arc<Semaphore>,
    pub hasher: HashAlgorithm
}

impl AuthState {
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        email_client: EmailClient,
        token_cipher: Arc<TokenCipher>
    ) -> Self {
        let max_concurrency = get_password_verify_max_concurrency();
        let hasher = common::constants::HASHER;

        Self {
            db,
            redis,
            email_client,
            token_cipher,
            password_verify_semaphore: Arc::new(Semaphore::new(max_concurrency)),
            hasher
        }
    }
}
