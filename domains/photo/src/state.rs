use std::sync::Arc;

use common::utils::TokenCipher;
use deadpool_redis::Pool;
use oss::S3Client;
use sea_orm::DatabaseConnection;

pub struct PhotoState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub s3_client: Arc<S3Client>,
    pub token_cipher: Arc<TokenCipher>,
}

impl PhotoState {
    /// 创建照片模块状态实例
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis 连接池
    /// - `s3_client`: OSS 存储客户端
    /// - `face_tx`: 人脸识别任务发送通道（仅 `face_recognition` feature 启用时）
    /// - `token_cipher`: 图片访问令牌加密器
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        s3_client: Arc<S3Client>,
        token_cipher: Arc<TokenCipher>,
    ) -> Self {
        Self {
            db,
            redis,
            s3_client,
            token_cipher,
        }
    }
}
