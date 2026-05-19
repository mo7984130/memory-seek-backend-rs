use deadpool_redis::Pool;
use oss::S3Client;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use common::utils::TokenCipher;

/// 用户模块共享状态，包含数据库、缓存、存储和加密等依赖
pub struct UserState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub token_cipher: Arc<TokenCipher>,
    pub s3_client: Arc<S3Client>,
}

impl UserState {
    /// 创建新的用户模块共享状态
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis 连接池
    /// - `s3_client`: S3 对象存储客户端
    /// - `token_cipher`: 令牌加密工具
    ///
    /// # 返回
    /// 返回初始化后的 `UserState` 实例
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
