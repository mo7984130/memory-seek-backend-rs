use audit::{AuditEvent, AuditService};
use common::{
    db_transaction,
    error::{AppError, contextual::Result},
    ext::UintExt,
    models::CursorPage,
};
use types::photo::dto::collection::{CollectionCreateParam, CollectionUpdateParam};
use types::{auth::user::UserId, photo::collection::CollectionId};

use crate::mappers::{
    collection_mapper::CollectionMapper, collection_photo_mapper::CollectionPhotoMapper,
};
use crate::state::PhotoState;

pub(crate) struct CollectionRepo;

impl CollectionRepo {
    /// 查询用户可见的照片所属相册 ID.
    pub(crate) async fn query_collection_ids_by_photo(
        state: &PhotoState,
        user_id: UserId,
        photo_id: types::photo::photo::PhotoId,
    ) -> common::error::contextual::Result<Vec<CollectionId>> {
        CollectionPhotoMapper::query_collection_ids_by_photo_id(&state.db, user_id, photo_id).await
    }
    /// 批量查询相册的 ID 和名称摘要.
    pub(crate) async fn query_collection_briefs(
        state: &PhotoState,
        ids: &[CollectionId],
    ) -> common::error::contextual::Result<Vec<(CollectionId, String)>> {
        CollectionMapper::query_id_and_name_by_ids(&state.db, ids).await
    }
    /// 按游标查询相册中的照片 ID.
    pub(crate) async fn query_collection_photo_ids(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        req: &types::photo::dto::collection::CollectionPhotoCursorPageParam,
    ) -> common::error::contextual::Result<CursorPage<types::photo::photo::PhotoId, ()>> {
        CollectionPhotoMapper::query_photo_id_by_collection_id(
            &state.db,
            user_id,
            collection_id,
            req.cursor.as_ref(),
            req.size,
        )
        .await
    }
    /// 校验归属后批量添加照片到相册.
    pub(crate) async fn add_collection_photos(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: &types::photo::models::PhotoIds,
    ) -> Result<u64> {
        CollectionMapper::ensure_belong(&state.db, user_id, collection_id).await?;
        let photo_ids = photo_ids.clone();
        db_transaction!(scoped & state.db, |txn| {
            let count =
                CollectionMapper::add_photos_batch(txn, user_id, collection_id, &photo_ids).await?;
            if let Some(photo_id) = photo_ids.first() {
                let file_id =
                    crate::mappers::photo_mapper::PhotoMapper::query_file_id_by_id(txn, *photo_id)
                        .await?;
                CollectionMapper::update_cover_photo(txn, collection_id, Some(*photo_id), file_id)
                    .await?;
            }
            AuditService::append_many(
                txn,
                photo_ids.iter().map(|photo_id| {
                    AuditEvent::new("collect")
                        .with_actor(user_id.0)
                        .with_target("photo", photo_id.0)
                        .with_detail(serde_json::json!({ "collectionId": collection_id.0 }))
                }),
            )
            .await?;
            Ok(count)
        })
        .await
    }
    /// 校验归属后批量移除相册中的照片.
    pub(crate) async fn remove_collection_photos(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: &types::photo::models::PhotoIds,
    ) -> Result<u64> {
        let photo_ids = photo_ids.clone();
        db_transaction!(scoped & state.db, |txn| {
            let collection =
                CollectionMapper::ensure_belong_with_return(txn, user_id, collection_id).await?;
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
                .records
                .first()
                {
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
            }
            CollectionMapper::update_photo_count_delta(txn, collection_id, -(rows as i64)).await?;
            AuditService::append_many(
                txn,
                photo_ids.iter().map(|photo_id| {
                    AuditEvent::new("uncollect")
                        .with_actor(user_id.0)
                        .with_target("photo", photo_id.0)
                        .with_detail(serde_json::json!({ "collectionId": collection_id.0 }))
                }),
            )
            .await?;
            Ok(rows)
        })
        .await
    }
    /// 查询用户的相册列表.
    pub(crate) async fn query_collections(
        state: &PhotoState,
        user_id: UserId,
    ) -> common::error::contextual::Result<Vec<types::photo::collection::CollectionRecord>> {
        CollectionMapper::query_by_user_id(&state.db, user_id).await
    }

    /// 创建用户相册.
    pub(crate) async fn create_collection(
        state: &PhotoState,
        user_id: UserId,
        req: CollectionCreateParam,
    ) -> common::error::contextual::Result<types::photo::collection::CollectionRecord> {
        db_transaction!(scoped & state.db, |txn| {
            let collection =
                CollectionMapper::insert(txn, user_id, req.name, req.description).await?;
            AuditService::append(
                txn,
                AuditEvent::new("photo.collection_created")
                    .with_actor(user_id.0)
                    .with_target("collection", collection.id.0),
            )
            .await?;
            Ok(collection)
        })
        .await
    }

    /// 校验归属后更新相册信息.
    pub(crate) async fn update_collection(
        state: &PhotoState,
        user_id: UserId,
        id: CollectionId,
        req: CollectionUpdateParam,
    ) -> Result<()> {
        let rows = db_transaction!(scoped & state.db, |txn| {
            let rows =
                CollectionMapper::update_info(txn, id, user_id, req.name, req.description).await?;
            if rows > 0 {
                AuditService::append(
                    txn,
                    AuditEvent::new("photo.collection_updated")
                        .with_actor(user_id.0)
                        .with_target("collection", id.0),
                )
                .await?;
            }
            Ok(rows)
        })
        .await?;
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
    pub(crate) async fn delete_collection(
        state: &PhotoState,
        user_id: UserId,
        id: CollectionId,
    ) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            CollectionMapper::delete_by_id(txn, id, user_id)
                .await?
                .no_zero_or_warn(
                    "delete_collection_fail",
                    "删除收藏夹失败",
                    AppError::bad_request("删除收藏夹失败"),
                )?;
            CollectionPhotoMapper::delete_by_collection_id(txn, id, user_id).await?;
            AuditService::append(
                txn,
                AuditEvent::new("photo.collection_deleted")
                    .with_actor(user_id.0)
                    .with_target("collection", id.0),
            )
            .await?;
            Ok(())
        })
        .await
    }
}
