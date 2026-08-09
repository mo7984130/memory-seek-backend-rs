use common::cache::MultiLevelCache;
use deadpool_redis::Pool;
use oss::S3Client;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use std::time::Duration;

use crate::models::UserInfoRow;

/// 用户模块共享状态，包含数据库、缓存、存储等依赖
pub struct UserState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    /// 用户信息三级缓存（本地 moka → Redis → 数据库）
    pub cache_user_info: MultiLevelCache<UserInfoRow>,
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
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        cache_local_capacity: u64,
        cache_local_ttl_secs: u64,
        s3_client: Arc<S3Client>,
    ) -> Self {
        let cache_user_info = MultiLevelCache::new(
            "user_info",
            redis.clone(),
            cache_local_capacity,
            Duration::from_secs(cache_local_ttl_secs),
        );
        Self {
            db,
            redis,
            cache_user_info,
            s3_client,
        }
    }
}
