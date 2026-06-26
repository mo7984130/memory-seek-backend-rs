use crate::mappers::collection_mapper::CollectionMapper;
use crate::mappers::collection_photo_mapper::CollectionPhotoMapper;
use crate::models::collection::CollectionResult;
use crate::state::PhotoState;
use common::Result;
use common::error::AppError;
use common::ext::{CacheExtension, OkExt, ToErr, log_warn};
use common::utils::DbUtils;
use common::{metrics_group, metrics_success, metrics_timer_name, timed, utils::MetricsTimerExt};
use constants::RedisKeys;
use entities::auth::user::UserId;
use entities::photo::collection::{CollectionId, CollectionRecord};
use moka::future::Cache;
use once_cell::sync::Lazy;

pub(crate) struct CollectionService;

// 定义全局缓存：Key 为 user_id (i64), Value 为 collection_id (i64)
// 设置最大容量 1024 * 16 条，过期时间 24 小时
static LOCAL_FAVORITE_ID_CACHE: Lazy<Cache<UserId, CollectionId>> = Lazy::new(|| {
    Cache::builder()
        .max_capacity(1024 * 16)
        .time_to_live(std::time::Duration::from_secs(24 * 60 * 60))
        .build()
});

// 查询
impl CollectionService {
    pub async fn get_favorite_collection_id(
        state: &PhotoState,
        user_id: UserId,
    ) -> Result<CollectionId> {
        // 先从本地缓存获取
        if let Some(id) = LOCAL_FAVORITE_ID_CACHE.get(&user_id).await {
            return Ok(id);
        }

        // 从redis中获取
        let id = state
            .redis
            .get_or_load(
                RedisKeys::photo::collection::favorite_collection_id(user_id),
                24 * 60 * 60,
                || async move {
                    // 从数据库中获取
                    let collection_id =
                        CollectionMapper::query_favorite_collection_id(&state.db, user_id).await?;
                    match collection_id {
                        Some(id) => id,
                        None => Self::create_favorite_collection(state, user_id).await?.id,
                    }
                    .to_ok()
                },
            )
            .await?;

        // 回填本地缓存
        LOCAL_FAVORITE_ID_CACHE.insert(user_id, id).await;

        Ok(id)
    }

    pub async fn get_collection_list(
        state: &PhotoState,
        user_id: UserId,
    ) -> Result<Vec<CollectionResult>> {
        metrics_group!("get_collection_list");

        // 获取用户收藏夹
        let collections = CollectionMapper::query_by_user_id(&state.db, user_id)
            .timed(metrics_timer_name!("get_collection_list", "query_by_user_id"))
            .await?;

        // 如果收藏夹为空, 创建默认的我喜欢收藏夹
        if collections.is_empty() {
            let record = Self::create_favorite_collection(state, user_id).await?;
            let vo = CollectionResult::from(record);
            metrics_success!("get_collection_list");
            return Ok(vec![vo]);
        }

        // 组装结果
        let result: Vec<CollectionResult> = collections
            .into_iter()
            .map(|c| CollectionResult::from(c).with_generate_cover_token(&state.token_cipher))
            .collect();

        metrics_success!("get_collection_list");
        Ok(result)
    }
}

// 添加
impl CollectionService {
    pub async fn create_collection(
        state: &PhotoState,
        user_id: UserId,
        name: String,
        description: Option<String>,
        is_favorite: bool,
    ) -> Result<CollectionResult> {
        metrics_group!("create_collection");

        let collection = CollectionMapper::insert(&state.db, user_id, name, description, is_favorite)
            .timed(metrics_timer_name!("create_collection", "db_insert"))
            .await?;

        metrics_success!("create_collection");
        CollectionResult::from(collection).to_ok()
    }

    async fn create_favorite_collection(
        state: &PhotoState,
        user_id: UserId,
    ) -> Result<CollectionRecord> {
        CollectionMapper::insert(&state.db, user_id, "我喜欢".into(), None, true).await
    }
}

// 修改
impl CollectionService {
    pub async fn update_collection_info(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<()> {
        metrics_group!("update_collection_info");

        // 修改时鉴权
        CollectionMapper::update_info(&state.db, collection_id, user_id, name, description)
            .timed(metrics_timer_name!("update_collection_info", "db_update"))
            .await?;

        metrics_success!("update_collection_info");
        Ok(())
    }
}

// 删除
impl CollectionService {
    pub async fn delete_collection(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
    ) -> Result<()> {
        metrics_group!("delete_collection");

        // 我喜欢收藏夹不可以删除
        if Self::get_favorite_collection_id(state, user_id).await? == collection_id {
            return log_warn(
                "try_del_favorite",
                "用户尝试删除我喜欢文件夹",
                "",
                AppError::bad_request("我喜欢收藏夹不可删除"),
            )
            .to_err();
        }

        // 删除收藏夹 和 收藏夹照片
        timed!("delete_collection", "db_transaction", {
            DbUtils::write(&state.db, |txn| {
                Box::pin(async move {
                    // 删除收藏夹里面的照片
                    CollectionPhotoMapper::delete_by_collection_id(txn, collection_id, user_id)
                        .await?;
                    // 删除收藏夹本身
                    CollectionMapper::delete_by_id(txn, collection_id, user_id).await?;
                    Ok(())
                })
            })
            .await
        })?;

        metrics_success!("delete_collection");
        Ok(())
    }
}
