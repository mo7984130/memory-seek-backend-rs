use common::{
    error::{AppError, contextual::Result},
    ext::UintExt,
    models::CursorPage,
};
use types::photo::dto::collection::{CollectionCreateParam, CollectionUpdateParam};
use types::{auth::user::UserId, photo::collection::CollectionId};

use super::PhotoRepo;
use crate::mappers::{
    collection_mapper::CollectionMapper, collection_photo_mapper::CollectionPhotoMapper,
};

impl PhotoRepo {
    /// 查询用户可见的照片所属相册 ID.
    pub(crate) async fn query_collection_ids_by_photo(
        &self,
        user_id: UserId,
        photo_id: types::photo::photo::PhotoId,
    ) -> common::error::contextual::Result<Vec<CollectionId>> {
        CollectionPhotoMapper::query_collection_ids_by_photo_id(&self.db, user_id, photo_id).await
    }
    /// 批量查询相册的 ID 和名称摘要.
    pub(crate) async fn query_collection_briefs(
        &self,
        ids: &[CollectionId],
    ) -> common::error::contextual::Result<Vec<(CollectionId, String)>> {
        CollectionMapper::query_id_and_name_by_ids(&self.db, ids).await
    }
    /// 按游标查询相册中的照片 ID.
    pub(crate) async fn query_collection_photo_ids(
        &self,
        user_id: UserId,
        collection_id: CollectionId,
        req: &types::photo::dto::collection::CollectionPhotoCursorPageParam,
    ) -> common::error::contextual::Result<CursorPage<types::photo::photo::PhotoId, ()>> {
        Ok(CursorPage::from_oversize(
            CollectionPhotoMapper::query_photo_id_by_collection_id(
                &self.db,
                user_id,
                collection_id,
                req.cursor.as_ref(),
                req.size,
            )
            .await?,
            req.size,
        ))
    }
    /// 校验归属后批量添加照片到相册.
    pub(crate) async fn add_collection_photos(
        &self,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: &types::photo::models::PhotoIds,
    ) -> Result<u64> {
        CollectionMapper::ensure_belong(&self.db, user_id, collection_id).await?;
        let photo_ids = photo_ids.clone();
        self.transaction(|txn| {
            Box::pin(async move {
                let count =
                    CollectionMapper::add_photos_batch(txn, user_id, collection_id, &photo_ids)
                        .await?;
                if let Some(photo_id) = photo_ids.first() {
                    let file_id = crate::mappers::photo_mapper::PhotoMapper::query_file_id_by_id(
                        txn, *photo_id,
                    )
                    .await?;
                    CollectionMapper::update_cover_photo(
                        txn,
                        collection_id,
                        Some(*photo_id),
                        file_id,
                    )
                    .await?;
                }
                Ok(count)
            })
        })
        .await
    }
    /// 校验归属后批量移除相册中的照片.
    pub(crate) async fn remove_collection_photos(
        &self,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: &types::photo::models::PhotoIds,
    ) -> Result<u64> {
        let photo_ids = photo_ids.clone();
        self.transaction(|txn| {
            Box::pin(async move {
                let collection =
                    CollectionMapper::ensure_belong_with_return(txn, user_id, collection_id)
                        .await?;
                let cover_removed = collection
                    .cover_photo_id
                    .is_some_and(|id| photo_ids.contains(&id));
                let rows = CollectionPhotoMapper::delete_by_collection_id_and_photo_ids(
                    txn,
                    user_id,
                    collection_id,
                    &photo_ids,
                )
                .await?;
                if cover_removed {
                    if let Some(photo_id) = CollectionPhotoMapper::query_photo_id_by_collection_id(
                        txn,
                        user_id,
                        collection_id,
                        None,
                        1,
                    )
                    .await?
                    .first()
                    {
                        let file_id =
                            crate::mappers::photo_mapper::PhotoMapper::query_file_id_by_id(
                                txn, *photo_id,
                            )
                            .await?;
                        CollectionMapper::update_cover_photo(
                            txn,
                            collection_id,
                            Some(*photo_id),
                            file_id,
                        )
                        .await?;
                    }
                }
                CollectionMapper::update_photo_count_delta(txn, collection_id, -(rows as i64))
                    .await?;
                Ok(rows)
            })
        })
        .await
    }
    /// 查询用户的相册列表.
    pub(crate) async fn query_collections(
        &self,
        user_id: UserId,
    ) -> common::error::contextual::Result<Vec<types::photo::collection::CollectionRecord>> {
        CollectionMapper::query_by_user_id(&self.db, user_id).await
    }

    /// 创建用户相册.
    pub(crate) async fn create_collection(
        &self,
        user_id: UserId,
        req: CollectionCreateParam,
    ) -> common::error::contextual::Result<types::photo::collection::CollectionRecord> {
        CollectionMapper::insert(&self.db, user_id, req.name, req.description).await
    }

    /// 校验归属后更新相册信息.
    pub(crate) async fn update_collection(
        &self,
        user_id: UserId,
        id: CollectionId,
        req: CollectionUpdateParam,
    ) -> Result<()> {
        let rows =
            CollectionMapper::update_info(&self.db, id, user_id, req.name, req.description).await?;
        if rows == 0 {
            return Err(common::error::ContextualError::warn_without_source(
                "collection_update_info_fail",
                "修改收藏夹信息失败",
                AppError::bad_request("修改收藏夹信息失败"),
            ));
        }
        Ok(())
    }

    /// 校验归属后删除相册及其照片关联.
    pub(crate) async fn delete_collection(&self, user_id: UserId, id: CollectionId) -> Result<()> {
        self.transaction(|txn| {
            Box::pin(async move {
                CollectionMapper::delete_by_id(txn, id, user_id)
                    .await?
                    .no_zero_or_warn(
                        "delete_collection_fail",
                        "删除收藏夹失败",
                        AppError::bad_request("删除收藏夹失败"),
                    )?;
                CollectionPhotoMapper::delete_by_collection_id(txn, id, user_id).await?;
                Ok(())
            })
        })
        .await
    }
}
