use crate::mappers::{
    collection_mapper::CollectionMapper, collection_photo_mapper::CollectionPhotoMapper,
};
use crate::repo::CollectionRepo;
use crate::state::PhotoState;
use common::Result;
use common::ext::ToOk;
use common::utils::token_cipher;
use types::auth::user::UserId;
use types::photo::collection::CollectionId;
use types::photo::dto::collection::{CollectionCreateParam, CollectionUpdateParam, CollectionView};

pub(crate) struct CollectionService;

// 查询
impl CollectionService {
    /// 查询用户收藏夹.
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn get_collection_list(
        state: &PhotoState,
        user_id: UserId,
    ) -> Result<Vec<CollectionView>> {
        // 获取用户收藏夹
        let collections = CollectionRepo::query_collections(state, user_id).await?;

        // 组装结果
        let result = collections
            .into_iter()
            .map(|c| CollectionView::from(c).with_generate_cover_token(user_id, token_cipher()))
            .collect::<common::error::contextual::Result<Vec<_>>>()?;

        Ok(result)
    }
}

// 添加
impl CollectionService {
    /// 创建收藏夹.
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn create_collection(
        state: &PhotoState,
        user_id: UserId,
        req: CollectionCreateParam,
    ) -> Result<CollectionView> {
        let collection = CollectionRepo::create_collection(state, user_id, req).await?;

        CollectionView::from(collection).to_ok()
    }
}

// 修改
impl CollectionService {
    /// 更新收藏夹信息
    #[common_macros::metered]
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
        // 修改时鉴权
        CollectionRepo::update_collection(state, user_id, collection_id, req).await?;

        Ok(())
    }
}

// 删除
impl CollectionService {
    /// 删除收藏夹.
    #[common_macros::metered]
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, collection_id = %collection_id)
    )]
    pub async fn delete_collection(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
    ) -> Result<()> {
        // 删除收藏夹 和 收藏夹照片
        CollectionRepo::delete_collection(state, user_id, collection_id).await?;

        Ok(())
    }
}

// 当照片删除时
#[step_derive::declare_transaction_step(
    ctx = crate::services::photo_service::PhotoDeleteContext,
    slice = crate::services::photo_service::PHOTO_DELETE_STEPS,
    name = "collection_cleanup",
    owns = ["CollectionPhotoMapper", "CollectionMapper"],
)]
impl CollectionService {
    /// 清理收藏夹照片 和 更新收藏夹计数.
    async fn on_photo_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::photo_service::PhotoDeleteContext,
    ) -> common::error::contextual::Result<()> {
        let photo_ids = ctx.photo_ids();
        let affected = CollectionPhotoMapper::delete_by_photo_ids(txn, &photo_ids).await?;
        CollectionMapper::update_photo_count_delta_batch(txn, &affected).await?;
        Ok(())
    }
}
