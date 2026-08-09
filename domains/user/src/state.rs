use deadpool_redis::Pool;
use oss::S3Client;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// 用户模块共享状态，包含数据库、缓存、存储等依赖
pub struct UserState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub s3_client: Arc<S3Client>,
}

impl UserState {
    /// 创建新的用户模块共享状态
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis 连接池
    /// - `s3_client`: S3 对象存储客户端
    ///
    /// # 返回
    /// 返回初始化后的 `UserState` 实例
    pub fn new(db: DatabaseConnection, redis: Pool, s3_client: Arc<S3Client>) -> Self {
        Self {
            db,
            redis,
            s3_client,
        }
    }
}
