use crate::mappers::collection_mapper::CollectionMapper;
use crate::mappers::collection_photo_mapper::CollectionPhotoMapper;
use crate::state::PhotoState;
use common::Result;
use common::error::AppError;
use common::ext::{OkExt, UintExt};
use common::utils::token_cipher;
use common::{
    db_transaction, metrics_group, metrics_name, metrics_success, utils::MetricsTimerExt,
};
use types::auth::user::UserId;
use types::photo::collection::CollectionId;
use types::photo::dto::collection::{CollectionCreateParam, CollectionUpdateParam, CollectionView};

pub(crate) struct CollectionService;

// 查询
impl CollectionService {
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn get_collection_list(
        state: &PhotoState,
        user_id: UserId,
    ) -> Result<Vec<CollectionView>> {
        metrics_group!();

        // 获取用户收藏夹
        let collections = CollectionMapper::query_by_user_id(&state.db, user_id)
            .timed(metrics_name!("query_by_user_id"))
            .await?;

        // 组装结果
        let result: Vec<CollectionView> = collections
            .into_iter()
            .map(|c| CollectionView::from(c).with_generate_cover_token(user_id, token_cipher()))
            .collect();

        metrics_success!();
        Ok(result)
    }
}

// 添加
impl CollectionService {
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn create_collection(
        state: &PhotoState,
        user_id: UserId,
        req: CollectionCreateParam,
    ) -> Result<CollectionView> {
        metrics_group!();

        let CollectionCreateParam { name, description } = req;
        let collection = CollectionMapper::insert(&state.db, user_id, name, description)
            .timed(metrics_name!("db_insert"))
            .await?;

        metrics_success!();
        CollectionView::from(collection).to_ok()
    }
}

// 修改
impl CollectionService {
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, collection_id = %collection_id)
    )]
    pub async fn update_collection_info(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        req: CollectionUpdateParam,
    ) -> Result<()> {
        metrics_group!();

        // 修改时鉴权
        CollectionMapper::update_info(&state.db, collection_id, user_id, req.name, req.description)
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
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, collection_id = %collection_id)
    )]
    pub async fn delete_collection(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
    ) -> Result<()> {
        metrics_group!();

        // 删除收藏夹 和 收藏夹照片
        db_transaction!(&state.db, |txn| {
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
        .timed(metrics_name!("db_transaction"))
        .await?;

        metrics_success!();
        Ok(())
    }
}

// 照片删除步骤:收藏夹清理
#[step_derive::declare_step(
    ctx = crate::services::photo_service::PhotoDeleteContext,
    slice = crate::services::photo_service::PHOTO_DELETE_STEPS,
    name = "collection_cleanup",
    owns = ["CollectionPhotoMapper", "CollectionMapper"],
)]
impl CollectionService {
    async fn on_photo_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::photo_service::PhotoDeleteContext,
    ) -> common::Result<()> {
        let photo_ids = ctx.photo_ids();
        let affected = CollectionPhotoMapper::delete_by_photo_ids(txn, &photo_ids).await?;
        CollectionMapper::update_photo_count_delta_batch(txn, &affected).await?;
        Ok(())
    }
}
