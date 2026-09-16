use crate::mappers::{
    collection_mapper::CollectionMapper, collection_visual_mapper::CollectionVisualMapper,
};
use crate::repo::CollectionRepo;
use crate::state::VisualState;
use common::Result;
use common::ext::ToOk;
use types::auth::user::UserId;
use types::visual::collection::CollectionId;
use types::visual::dto::collection::{CollectionCreateParam, CollectionUpdateParam, CollectionView};

pub(crate) struct CollectionService;

// 查询
impl CollectionService {
    /// 查询用户收藏夹.
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn get_collection_list(
        state: &VisualState,
        user_id: UserId,
    ) -> Result<Vec<CollectionView>> {
        // 获取用户收藏夹
        let collections = CollectionRepo::query_collections(state, user_id).await?;

        // 组装结果
        let result = collections
            .into_iter()
            .map(|c| CollectionView::from(c).with_generate_cover_token(user_id))
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
        state: &VisualState,
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
        state: &VisualState,
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
        state: &VisualState,
        user_id: UserId,
        collection_id: CollectionId,
    ) -> Result<()> {
        // 删除收藏夹 和 收藏夹影像
        CollectionRepo::delete_collection(state, user_id, collection_id).await?;

        Ok(())
    }
}

// 当影像删除时
#[step_derive::declare_transaction_step(
    ctx = crate::services::visual_service::VisualDeleteContext,
    slice = crate::services::visual_service::MEDIA_DELETE_STEPS,
    name = "collection_cleanup",
    owns = ["CollectionVisualMapper", "CollectionMapper"],
    method = on_visual_delete,
)]
impl CollectionService {
    /// 清理收藏夹影像 和 更新收藏夹计数.
    async fn on_visual_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::visual_service::VisualDeleteContext,
    ) -> common::error::contextual::Result<()> {
        let visual_ids = ctx.visual_ids();
        let affected = CollectionVisualMapper::delete_by_visual_ids(txn, &visual_ids).await?;
        CollectionMapper::update_visual_count_delta_batch(txn, &affected).await?;
        Ok(())
    }
}
