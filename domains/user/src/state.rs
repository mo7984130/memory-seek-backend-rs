use deadpool_redis::Pool;
use multi_level_cache::CacheConfig;
use oss::S3Client;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::repo::UserRepo;

/// 用户模块共享状态，包含数据访问仓储、Redis、存储等依赖
pub struct UserState {
    /// 用户数据访问仓储，封装数据库与三级缓存
    pub repo: UserRepo,
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
    /// - `cache_config`: 缓存配置（启用开关、L1 容量、L1 TTL）
    ///
    /// # 返回
    /// 返回初始化后的 `UserState` 实例
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        cache_config: CacheConfig,
        s3_client: Arc<S3Client>,
    ) -> Self {
        Self {
            repo: UserRepo::new(db, redis.clone(), cache_config),
            redis,
            s3_client,
        }
    }
}
