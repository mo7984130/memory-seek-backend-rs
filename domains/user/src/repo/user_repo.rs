use audit::{AuditEvent, AuditService};
use common::error::{ContextualError, contextual::Result};
use common::ext::RedisExt;
use common::utils::MetricsTimerExt;
use common::{db_transaction, metrics_name};
use constants::RedisKeys;
use deadpool_redis::Pool;
use multi_level_cache::{CacheConfig, MultiLevelCache};
use sea_orm::DatabaseConnection;
use types::auth::user::UserId;
use types::user::UserInfo;

use crate::config::USER_INFO_CACHE_TTL;
use crate::error_ext::ContextualErrorExt;
use crate::mapper::UserMapper;
use crate::models::UserBriefRow;

/// 用户数据访问仓储，封装数据库与缓存，向 service 层提供统一的数据访问入口
pub struct UserRepo {
    db: DatabaseConnection,
    redis: Pool,
    cache_user_info: MultiLevelCache<UserBriefRow, ContextualError>,
    cache_user_info_single: MultiLevelCache<UserInfo, ContextualError>,
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
    pub async fn get_user_info(&self, user_id: UserId) -> Result<UserInfo> {
        let info = self
            .cache_user_info_single
            .get_or_load(
                &RedisKeys::auth::user_info_cache(user_id),
                USER_INFO_CACHE_TTL,
                || async move {
                    let user = UserMapper::query_by_id(&self.db, user_id)
                        .await?
                        .user_not_found()?;
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
    ) -> Result<Vec<Option<UserBriefRow>>> {
        let result: Vec<Option<UserBriefRow>> = self
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
    pub async fn change_nickname(&self, user_id: UserId, new_nickname: String) -> Result<String> {
        let nickname_for_update = new_nickname.clone();
        db_transaction!(contextual & self.db, |txn| {
            UserMapper::update_nickname(txn, user_id, &nickname_for_update)
                .timed(metrics_name!("db_update"))
                .await?;
            AuditService::append(
                txn,
                AuditEvent::new("user.nickname_changed")
                    .with_actor(user_id.0)
                    .with_target("user", user_id.0),
            )
            .await
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        self.invalidate_user_info(user_id).await;

        Ok(new_nickname)
    }

    /// 在事务内更新头像，返回旧头像 key（由调用方决定是否删除旧文件）
    pub async fn update_avatar(&self, user_id: UserId, new_key: String) -> Result<Option<String>> {
        let old_key = db_transaction!(contextual & self.db, |txn| {
            let old_key = UserMapper::update_avatar(txn, user_id, new_key).await?;
            AuditService::append(
                txn,
                AuditEvent::new("user.avatar_updated")
                    .with_actor(user_id.0)
                    .with_target("user", user_id.0),
            )
            .await?;
            Ok(old_key)
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        self.invalidate_user_info(user_id).await;

        Ok(old_key)
    }

    /// 查询用户密码哈希
    pub async fn query_password_hash(&self, user_id: UserId) -> Result<String> {
        UserMapper::query_password_hash(&self.db, user_id)
            .timed(metrics_name!("db_query"))
            .await
    }

    /// 更新用户密码哈希
    pub async fn update_password(&self, user_id: UserId, new_password_hash: String) -> Result<()> {
        db_transaction!(contextual & self.db, |txn| {
            UserMapper::update_password(txn, user_id, new_password_hash).await?;
            AuditService::append(
                txn,
                AuditEvent::new("user.password_changed")
                    .with_actor(user_id.0)
                    .with_target("user", user_id.0),
            )
            .await
        })
        .timed(metrics_name!("db_transaction"))
        .await
    }

    /// 清除 refresh_token，并在同一事务中追加登出审计事件
    pub async fn clear_refresh_token(&self, user_id: UserId) -> Result<()> {
        db_transaction!(contextual & self.db, |txn| {
            UserMapper::clear_refresh_token(txn, user_id).await?;
            AuditService::append(
                txn,
                AuditEvent::new("user.logged_out")
                    .with_actor(user_id.0)
                    .with_target("user", user_id.0),
            )
            .await
        })
        .timed(metrics_name!("db_transaction"))
        .await
    }

    /// 登出：清除 refresh_token 与 access_token，并失效用户信息缓存
    pub async fn logout(&self, user_id: UserId) -> Result<()> {
        self.clear_refresh_token(user_id).await?;
        self.redis
            .del(RedisKeys::auth::user_access_token(user_id))
            .timed(metrics_name!("redis_delete"))
            .await?;

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
