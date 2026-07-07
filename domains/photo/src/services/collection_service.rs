use crate::mappers::collection_mapper::CollectionMapper;
use crate::mappers::collection_photo_mapper::CollectionPhotoMapper;
use crate::models::collection::CollectionResult;
use crate::state::PhotoState;
use common::Result;
use common::ext::OkExt;
use common::utils::DbUtils;
use common::{metrics_group, metrics_success, metrics_timer_name, timed, utils::MetricsTimerExt};
use entities::auth::user::UserId;
use entities::photo::collection::CollectionId;

pub(crate) struct CollectionService;

// 查询
impl CollectionService {
    pub async fn get_collection_list(
        state: &PhotoState,
        user_id: UserId,
    ) -> Result<Vec<CollectionResult>> {
        metrics_group!("get_collection_list");

        // 获取用户收藏夹
        let collections = CollectionMapper::query_by_user_id(&state.db, user_id)
            .timed(metrics_timer_name!(
                "get_collection_list",
                "query_by_user_id"
            ))
            .await?;

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
    ) -> Result<CollectionResult> {
        metrics_group!("create_collection");

        let collection =
            CollectionMapper::insert(&state.db, user_id, name, description)
                .timed(metrics_timer_name!("create_collection", "db_insert"))
                .await?;

        metrics_success!("create_collection");
        CollectionResult::from(collection).to_ok()
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
