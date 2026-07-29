use crate::mappers::collection_mapper::CollectionMapper;
use crate::mappers::collection_photo_mapper::CollectionPhotoMapper;
use crate::models::collection::CollectionResult;
use crate::state::PhotoState;
use common::Result;
use common::error::AppError;
use common::ext::{OkExt, UintExt};
use common::utils::DbUtils;
use common::{metrics_group, metrics_name, metrics_success, utils::MetricsTimerExt};
use types::auth::user::UserId;
use types::photo::collection::CollectionId;

pub(crate) struct CollectionService;

// 查询
impl CollectionService {
    pub async fn get_collection_list(
        state: &PhotoState,
        user_id: UserId,
    ) -> Result<Vec<CollectionResult>> {
        metrics_group!();

        // 获取用户收藏夹
        let collections = CollectionMapper::query_by_user_id(&state.db, user_id)
            .timed(metrics_name!("query_by_user_id"))
            .await?;

        // 组装结果
        let result: Vec<CollectionResult> = collections
            .into_iter()
            .map(|c| CollectionResult::from(c).with_generate_cover_token(&state.token_cipher))
            .collect();

        metrics_success!();
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
        metrics_group!();

        let collection = CollectionMapper::insert(&state.db, user_id, name, description)
            .timed(metrics_name!("db_insert"))
            .await?;

        metrics_success!();
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
        metrics_group!();

        // 修改时鉴权
        CollectionMapper::update_info(&state.db, collection_id, user_id, name, description)
            .timed(metrics_name!("db_update"))
            .await?
            .no_zero_or_warn(
                "collection_update_info_fail",
                "修改收藏夹信息失败",
                AppError::bad_request("修改收藏夹信息失败"),
            )?;

        metrics_success!();
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
        metrics_group!();

        // 删除收藏夹 和 收藏夹照片
        DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                // 删除收藏夹本身
                CollectionMapper::delete_by_id(txn, collection_id, user_id)
                    .await?
                    .no_zero_or_warn(
                        "delete_collection_fail",
                        "删除收藏夹失败",
                        AppError::bad_request("删除收藏夹失败"),
                    )?;

                // 删除收藏夹里面的照片
                CollectionPhotoMapper::delete_by_collection_id(txn, collection_id, user_id).await?;
                Ok(())
            })
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        metrics_success!();
        Ok(())
    }
}
