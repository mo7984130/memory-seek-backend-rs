use deadpool_redis::Pool;
use email::EmailClient;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use common::utils::TokenCipher;

/// 认证服务状态
pub struct AuthState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub email_client: EmailClient,
    pub token_cipher: Arc<TokenCipher>,
}

impl AuthState {
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        email_client: EmailClient,
        token_cipher: Arc<TokenCipher>,
    ) -> Self {
        Self {
            db,
            redis,
            email_client,
            token_cipher,
        }
    }
}
