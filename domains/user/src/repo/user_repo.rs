use std::time::Duration;

use common::ext::{OptionExt, RedisExt, ToOk};
use common::utils::{DbUtils, MetricsTimerExt};
use common::{Result, metrics_name};
use constants::RedisKeys;
use deadpool_redis::Pool;
use multi_level_cache::{CacheConfig, MultiLevelCache};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QuerySelect, Set,
};
use types::auth::user::{self, UserId, UserRecord};
use types::user::UserInfo;

use crate::config::USER_INFO_CACHE_TTL_SECS;
use crate::models::UserInfoRow;

/// 用户数据访问仓储，封装数据库与缓存，向 service 层提供统一的数据访问入口
pub struct UserRepo {
    db: DatabaseConnection,
    redis: Pool,
    cache_user_info: MultiLevelCache<UserInfoRow, common::error::AppError>,
    cache_user_info_single: MultiLevelCache<UserInfo, common::error::AppError>,
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
                RedisKeys::auth::user_info_cache(user_id).as_str(),
                Duration::from_secs(USER_INFO_CACHE_TTL_SECS as u64),
                || {
                    Box::pin(async move {
                        let user = Self::query_by_id(&self.db, user_id)
                            .await?
                            .ok_or_warn_bad_request("user_not_found", "用户不存在", "用户不存在")?;
                        Ok(UserInfo::from_with_token(user))
                    })
                },
            )
            .timed(metrics_name!("cache_get_or_load"))
            .await?;

        info.to_ok()
    }

    /// 批量获取多个用户的基本信息（带三级缓存），未找到的用户对应位置为 `None`
    pub async fn get_user_info_batch(
        &self,
        user_ids: &[UserId],
    ) -> Result<Vec<Option<UserInfoRow>>> {
        let result: Vec<Option<UserInfoRow>> = self
            .cache_user_info
            .get_or_load_batch(
                user_ids,
                |id| RedisKeys::auth::user_info_cache(*id),
                Duration::from_secs(USER_INFO_CACHE_TTL_SECS as u64),
                |miss_ids| {
                    Box::pin(async move {
                        let users: Vec<UserInfoRow> = user::Entity::find()
                            .filter(user::Column::Id.is_in(miss_ids))
                            .select_only()
                            .column_as(user::Column::Id, "user_id")
                            .column(user::Column::Nickname)
                            .column(user::Column::AvatarFileId)
                            .into_model::<UserInfoRow>()
                            .all(&self.db)
                            .await?;
                        Ok(users)
                    })
                },
                |dto| dto.user_id,
            )
            .timed(metrics_name!("cache_get_or_load_batch"))
            .await?;

        result.to_ok()
    }

    /// 修改用户昵称，并失效用户信息缓存（L1 + L2）
    pub async fn change_nickname(&self, user_id: UserId, new_nickname: String) -> Result<String> {
        user::Entity::update_many()
            .col_expr(user::Column::Nickname, Expr::value(new_nickname.clone()))
            .filter(user::Column::Id.eq(user_id))
            .exec(&self.db)
            .timed(metrics_name!("db_update"))
            .await?;

        self.invalidate_user_info(user_id).await;

        Ok(new_nickname)
    }

    /// 在事务内更新头像，返回旧头像 key（由调用方决定是否删除旧文件）
    pub async fn update_avatar(&self, user_id: UserId, new_key: String) -> Result<Option<String>> {
        let old_key = DbUtils::write(&self.db, move |txn| {
            let new_key = new_key.clone();
            Box::pin(async move {
                let old_key: Option<String> = user::Entity::find_by_id(user_id)
                    .select_only()
                    .column(user::Column::AvatarFileId)
                    .into_values::<Option<String>, user::Column>()
                    .one(txn)
                    .await?
                    .ok_or_warn_bad_request("user_not_found", "用户不存在", "用户不存在")?;

                user::ActiveModel {
                    id: Set(user_id),
                    avatar_file_id: Set(Some(new_key)),
                    ..Default::default()
                }
                .update(txn)
                .await?;

                Ok(old_key)
            })
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        self.invalidate_user_info(user_id).await;

        Ok(old_key)
    }

    /// 查询用户密码哈希
    pub async fn query_password_hash(&self, user_id: UserId) -> Result<String> {
        let password: String = user::Entity::find_by_id(user_id)
            .select_only()
            .column(user::Column::Password)
            .into_tuple()
            .one(&self.db)
            .timed(metrics_name!("db_query"))
            .await?
            .ok_or_warn_bad_request("user_not_found", "用户不存在", "用户不存在")?;

        password.to_ok()
    }

    /// 更新用户密码哈希
    pub async fn update_password(&self, user_id: UserId, new_password_hash: String) -> Result<()> {
        user::ActiveModel {
            id: Set(user_id),
            password: Set(new_password_hash),
            ..Default::default()
        }
        .update(&self.db)
        .timed(metrics_name!("db_update"))
        .await?;

        Ok(())
    }

    /// 登出：清除 refresh_token 与 access_token，并失效用户信息缓存
    pub async fn logout(&self, user_id: UserId) -> Result<()> {
        let (refresh_token_result, access_token_result) = tokio::join!(
            user::ActiveModel {
                id: Set(user_id),
                refresh_token: Set(None),
                refresh_token_expire_at: Set(None),
                ..Default::default()
            }
            .update(&self.db)
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

    /// 按 ID 查询完整用户记录（数据库直查，缓存未命中时由调用方回填）
    async fn query_by_id(db: &impl ConnectionTrait, user_id: UserId) -> Result<Option<UserRecord>> {
        user::Entity::find_by_id(user_id)
            .one(db)
            .await?
            .map(UserRecord::from)
            .to_ok()
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
