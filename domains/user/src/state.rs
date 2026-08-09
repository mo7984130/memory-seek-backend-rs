use common::cache::{CacheConfig, MultiLevelCache};
use deadpool_redis::Pool;
use oss::S3Client;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use types::user::UserInfo;

use crate::models::UserInfoRow;

/// 用户模块共享状态，包含数据库、缓存、存储等依赖
pub struct UserState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    /// 用户信息批量三级缓存（本地 moka → Redis → 数据库）
    pub cache_user_info: MultiLevelCache<UserInfoRow>,
    /// 用户信息单查三级缓存（缓存完整 UserInfo，含确定性加密的 token）
    pub cache_user_info_single: MultiLevelCache<UserInfo>,
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
        let cache_user_info = MultiLevelCache::new("user_info", redis.clone(), cache_config);
        let cache_user_info_single =
            MultiLevelCache::new("user_info_single", redis.clone(), cache_config);
        Self {
            db,
            redis,
            cache_user_info,
            cache_user_info_single,
            s3_client,
        }
    }
}
