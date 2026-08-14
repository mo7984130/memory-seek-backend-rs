use crate::mappers::{
    collection_mapper::CollectionMapper, collection_photo_mapper::CollectionPhotoMapper,
};
use crate::state::PhotoState;
use common::Result;
use common::ext::OkExt;
use common::utils::token_cipher;
use types::auth::user::UserId;
use types::photo::collection::CollectionId;
use types::photo::dto::collection::{CollectionCreateParam, CollectionUpdateParam, CollectionView};

pub(crate) struct CollectionService;

// 查询
impl CollectionService {
    /// 查询用户拥有的相册, 并生成封面访问令牌.
    #[common::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn get_collection_list(
        state: &PhotoState,
        user_id: UserId,
    ) -> Result<Vec<CollectionView>> {
        // 获取用户收藏夹
        let collections = state.repo.query_collections(user_id).await?;

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
    /// 创建相册并返回相册视图.
    #[common::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn create_collection(
        state: &PhotoState,
        user_id: UserId,
        req: CollectionCreateParam,
    ) -> Result<CollectionView> {
        let collection = state.repo.create_collection(user_id, req).await?;

        CollectionView::from(collection).to_ok()
    }
}

// 修改
impl CollectionService {
    /// 更新相册信息, 并在仓储层校验相册所有权.
    #[common::metered]
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
        state
            .repo
            .update_collection(user_id, collection_id, req)
            .await?;

        Ok(())
    }
}

// 删除
impl CollectionService {
    /// 删除相册及其照片关联关系.
    #[common::metered]
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
        state.repo.delete_collection(user_id, collection_id).await?;

        Ok(())
    }
}

// 照片删除步骤:收藏夹清理
#[step_derive::declare_step(
    ctx = crate::repo::photo_repo::PhotoDeleteContext,
    slice = crate::repo::photo_repo::PHOTO_DELETE_STEPS,
    name = "collection_cleanup",
    owns = ["CollectionPhotoMapper", "CollectionMapper"],
)]
impl CollectionService {
    /// 清理照片删除后失效的相册关联, 并同步照片计数.
    async fn on_photo_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::repo::photo_repo::PhotoDeleteContext,
    ) -> common::Result<()> {
        let photo_ids = ctx.photo_ids();
        let affected = CollectionPhotoMapper::delete_by_photo_ids(txn, &photo_ids).await?;
        CollectionMapper::update_photo_count_delta_batch(txn, &affected).await?;
        Ok(())
    }
}
