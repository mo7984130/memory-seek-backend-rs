use deadpool_redis::Pool;
use email::EmailClient;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::Semaphore;

use img_url_generator::EncryptionKey;

use crate::{config::{self, PASSWORD_VERIFY_MAX_CONCURRENCY}, utils::password::{HashAlgorithm}};

/// 认证服务状态
pub struct AuthState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub email_client: EmailClient,
    pub encryption_key: EncryptionKey,
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
        encryption_key: EncryptionKey
    ) -> Self {
        // 计算最大并发数：默认为 CPU 核心数的一半
        let max_concurrency = if PASSWORD_VERIFY_MAX_CONCURRENCY == 0 {
            (num_cpus::get() / 2).max(1)
        } else {
            PASSWORD_VERIFY_MAX_CONCURRENCY
        };
        // 配置hasher
        let hasher = config::HASHER;

        Self {
            db,
            redis,
            email_client,
            encryption_key,
            password_verify_semaphore: Arc::new(Semaphore::new(max_concurrency)),
            hasher
        }
    }
}
