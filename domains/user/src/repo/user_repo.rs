use common::error::{AppError, DeferredError, DeferredResult};
use common::ext::{DeferOptionExt, RedisExt};
use common::utils::MetricsTimerExt;
use common::{db_transaction, metrics_name};
use constants::RedisKeys;
use deadpool_redis::Pool;
use multi_level_cache::{CacheConfig, MultiLevelCache};
use sea_orm::DatabaseConnection;
use types::auth::user::UserId;
use types::user::UserInfo;

use crate::config::USER_INFO_CACHE_TTL;
use crate::mapper::UserMapper;
use crate::models::UserInfoRow;

/// 用户数据访问仓储，封装数据库与缓存，向 service 层提供统一的数据访问入口
pub struct UserRepo {
    db: DatabaseConnection,
    redis: Pool,
    cache_user_info: MultiLevelCache<UserInfoRow, DeferredError>,
    cache_user_info_single: MultiLevelCache<UserInfo, DeferredError>,
}

impl UserRepo {
    /// 创建用户数据访问仓储
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis 连接池（缓存 L2 存储与登出时删除 access_token）
    /// - `cache_config`: 缓存配置（启用开关、L1 容量、L1 TTL）
    pub fn new(db: DatabaseConnection, redis: Pool, cache_config: CacheConfig) -> Self {
        let cache_user_info =
            MultiLevelCache::new_with_name("user_info", redis.clone(), cache_config);
        let cache_user_info_single =
            MultiLevelCache::new_with_name("user_info_single", redis.clone(), cache_config);
        Self {
            db,
            redis,
            cache_user_info,
            cache_user_info_single,
        }
    }

    /// 获取用户完整信息（带三级缓存，缓存完整 `UserInfo`，token 确定性加密可安全缓存）
    pub async fn get_user_info(&self, user_id: UserId) -> DeferredResult<UserInfo> {
        let info = self
            .cache_user_info_single
            .get_or_load(
                RedisKeys::auth::user_info_cache(user_id).as_str(),
                USER_INFO_CACHE_TTL,
                || async move {
                    let user = UserMapper::query_by_id(&self.db, user_id)
                        .await?
                        .defer_warn_none(
                            "user_not_found",
                            "用户不存在",
                            AppError::bad_request("用户不存在"),
                        )?;
                    UserInfo::from_with_token(user)
                },
            )
            .timed(metrics_name!("cache_get_or_load"))
            .await?;

        Ok(info)
    }

    /// 批量获取多个用户的基本信息（带三级缓存），未找到的用户对应位置为 `None`
    pub async fn get_user_info_batch(
        &self,
        user_ids: &[UserId],
    ) -> DeferredResult<Vec<Option<UserInfoRow>>> {
        let result: Vec<Option<UserInfoRow>> = self
            .cache_user_info
            .get_or_load_batch(
                user_ids,
                |id| RedisKeys::auth::user_info_cache(*id),
                USER_INFO_CACHE_TTL,
                |miss_ids| async move {
                    let users = UserMapper::query_info_rows(&self.db, &miss_ids).await?;
                    Ok(users)
                },
                |dto| dto.user_id,
            )
            .timed(metrics_name!("cache_get_or_load_batch"))
            .await?;

        Ok(result)
    }

    /// 修改用户昵称，并失效用户信息缓存（L1 + L2）
    pub async fn change_nickname(
        &self,
        user_id: UserId,
        new_nickname: String,
    ) -> DeferredResult<String> {
        UserMapper::update_nickname(&self.db, user_id, &new_nickname)
            .timed(metrics_name!("db_update"))
            .await?;

        self.invalidate_user_info(user_id).await;

        Ok(new_nickname)
    }

    /// 在事务内更新头像，返回旧头像 key（由调用方决定是否删除旧文件）
    pub async fn update_avatar(
        &self,
        user_id: UserId,
        new_key: String,
    ) -> DeferredResult<Option<String>> {
        let old_key = db_transaction!(deferred & self.db, |txn| {
            UserMapper::update_avatar(txn, user_id, new_key).await
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        self.invalidate_user_info(user_id).await;

        Ok(old_key)
    }

    /// 查询用户密码哈希
    pub async fn query_password_hash(&self, user_id: UserId) -> DeferredResult<String> {
        UserMapper::query_password_hash(&self.db, user_id)
            .timed(metrics_name!("db_query"))
            .await
    }

    /// 更新用户密码哈希
    pub async fn update_password(
        &self,
        user_id: UserId,
        new_password_hash: String,
    ) -> DeferredResult<()> {
        UserMapper::update_password(&self.db, user_id, new_password_hash)
            .timed(metrics_name!("db_update"))
            .await
    }

    /// 登出：清除 refresh_token 与 access_token，并失效用户信息缓存
    pub async fn logout(&self, user_id: UserId) -> DeferredResult<()> {
        let (refresh_token_result, access_token_result) = tokio::join!(
            UserMapper::clear_refresh_token(&self.db, user_id)
                .timed(metrics_name!("db_update")),
            self.redis
                .del(RedisKeys::auth::user_access_token(user_id))
                .timed(metrics_name!("redis_delete"))
        );
        refresh_token_result?;
        access_token_result?;

        self.invalidate_user_info(user_id).await;

        Ok(())
    }

    /// 失效用户信息缓存（L1 + L2），失败不返回错误，下次读取时自动重建
    async fn invalidate_user_info(&self, user_id: UserId) {
        let cache_key = RedisKeys::auth::user_info_cache(user_id);
        let _ = tokio::join!(
            self.cache_user_info
                .invalidate(&cache_key)
                .timed(metrics_name!("cache_invalidate")),
            self.cache_user_info_single
                .invalidate(&cache_key)
                .timed(metrics_name!("cache_invalidate_single"))
        );
    }
}
