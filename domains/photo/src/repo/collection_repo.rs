use audit::{AuditEvent, AuditRecorder};
use common::{
    db_transaction,
    error::contextual::ext::UintExt,
    error::{AppError, ContextualError, contextual::Result},
    ext::ToErr,
    types::CursorPage,
};
use types::photo::{
    collection::CollectionRecord,
    dto::collection::{
        CollectionCreateParam, CollectionPhotoCursorPageParam, CollectionUpdateParam,
    },
};
use types::{
    auth::user::UserId, photo::collection::CollectionId, photo::models::PhotoIds,
    photo::photo::PhotoId,
};

use crate::mappers::{
    collection_mapper::CollectionMapper, collection_photo_mapper::CollectionPhotoMapper,
    photo_mapper::PhotoMapper,
};
use crate::state::PhotoState;

pub(crate) struct CollectionRepo;

impl CollectionRepo {
    /// 查询照片所属的收藏夹
    pub(crate) async fn query_collection_ids_by_photo(
        state: &PhotoState,
        user_id: UserId,
        photo_id: PhotoId,
    ) -> Result<Vec<CollectionId>> {
        CollectionPhotoMapper::query_collection_ids_by_photo_id(&state.db, user_id, photo_id).await
    }

    /// 查询收藏夹id 和 描述
    pub(crate) async fn query_collection_briefs(
        state: &PhotoState,
        ids: &[CollectionId],
    ) -> Result<Vec<(CollectionId, String)>> {
        CollectionMapper::query_id_and_name_by_ids(&state.db, ids).await
    }

    /// 游标查询收藏夹中的照片Id
    pub(crate) async fn query_collection_photo_ids(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        req: &CollectionPhotoCursorPageParam,
    ) -> Result<CursorPage<PhotoId, ()>> {
        CollectionPhotoMapper::query_photo_id_by_collection_id(
            &state.db,
            user_id,
            collection_id,
            req.cursor.as_ref(),
            req.size,
        )
        .await
    }

    pub async fn ensure_belong(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
    ) -> Result<()> {
        CollectionMapper::ensure_belong(&state.db, user_id, collection_id).await
    }

    pub async fn ensure_belong_with_return(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
    ) -> Result<CollectionRecord> {
        CollectionMapper::ensure_belong_with_return(&state.db, user_id, collection_id).await
    }

    /// 添加相册照片
    pub(crate) async fn add_collection_photos(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: &PhotoIds,
    ) -> Result<u64> {
        db_transaction!(scoped & state.db, |txn| {
            // 添加照片
            let count = CollectionMapper::add_photos_batch(
                txn,
                user_id,
                collection_id,
                photo_ids.iter().copied().collect::<Vec<_>>(),
            )
            .await?;

            // 修改封面
            if let Some(photo_id) = photo_ids.first() {
                let file_id = PhotoMapper::query_file_id_by_id(txn, *photo_id).await?;
                CollectionMapper::update_cover_photo(txn, collection_id, *photo_id, file_id)
                    .await?;
            }

            AuditRecorder::append(
                txn,
                AuditEvent::new("collect")
                    .with_actor(user_id.0)
                    .with_target("collection_id", collection_id.0)
                    .with_detail(serde_json::json!({ "photoIds": photo_ids })),
            )
            .await?;
            Ok(count)
        })
        .await
    }
    /// 移除收藏夹照片.
    pub(crate) async fn remove_collection_photos(
        state: &PhotoState,
        user_id: UserId,
        collection: CollectionRecord,
        photo_ids: &PhotoIds,
    ) -> Result<u64> {
        db_transaction!(scoped & state.db, |txn| {
            // 如果封面会被移除的话, 提前保存下来
            let cover_removed = collection
                .cover_photo_id
                .is_some_and(|id| photo_ids.contains(&id));

            // 删除收藏夹照片
            let rows = CollectionPhotoMapper::delete_by_collection_id_and_photo_ids(
                txn,
                user_id,
                collection.id,
                photo_ids,
            )
            .await?;

            // 计算封面
            if cover_removed {
                // 获取第一张照片
                if let Some(photo_id) = CollectionPhotoMapper::query_photo_id_by_collection_id(
                    txn,
                    user_id,
                    collection.id,
                    None,
                    1,
                )
                .await?
                .records
                .first()
                {
                    let file_id = PhotoMapper::query_file_id_by_id(txn, *photo_id).await?;
                    CollectionMapper::update_cover_photo(txn, collection.id, *photo_id, file_id)
                        .await?;
                }
            }

            // 更新计数
            CollectionMapper::update_photo_count_delta(txn, collection.id, -(rows as i64)).await?;

            AuditRecorder::append(
                txn,
                AuditEvent::new("uncollect")
                    .with_actor(user_id.0)
                    .with_target("collection", collection.id.0)
                    .with_detail(serde_json::json!({ "photoIds": photo_ids })),
            )
            .await?;
            Ok(rows)
        })
        .await
    }

    /// 查询收藏夹列表.
    pub(crate) async fn query_collections(
        state: &PhotoState,
        user_id: UserId,
    ) -> Result<Vec<CollectionRecord>> {
        CollectionMapper::query_by_user_id(&state.db, user_id).await
    }

    /// 创建收藏夹.
    pub(crate) async fn create_collection(
        state: &PhotoState,
        user_id: UserId,
        req: CollectionCreateParam,
    ) -> Result<CollectionRecord> {
        db_transaction!(scoped & state.db, |txn| {
            // 插入
            let collection =
                CollectionMapper::insert(txn, user_id, req.name, req.description).await?;

            AuditRecorder::append(
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

    /// 更新收藏夹信息.
    pub(crate) async fn update_collection(
        state: &PhotoState,
        user_id: UserId,
        id: CollectionId,
        req: CollectionUpdateParam,
    ) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            let affected =
                CollectionMapper::update_info(txn, id, user_id, req.name, req.description).await?;
            if affected > 0 {
                AuditRecorder::append(
                    txn,
                    AuditEvent::new("photo.collection_updated")
                        .with_actor(user_id.0)
                        .with_target("collection", id.0),
                )
                .await?;
                Ok(())
            } else {
                return ContextualError::warn_without_source(
                    "collection_update_info_fail",
                    "修改收藏夹信息失败",
                    AppError::bad_request("修改收藏夹信息失败"),
                )
                .to_err();
            }
        })
        .await?;

        Ok(())
    }

    /// 删除收藏夹.
    /// 无需健全, 在删除的时候保证user_id相等
    pub(crate) async fn delete_collection(
        state: &PhotoState,
        user_id: UserId,
        id: CollectionId,
    ) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            // 删除收藏夹
            CollectionMapper::delete_by_id(txn, id, user_id)
                .await?
                .no_zero_or_warn(
                    "delete_collection_fail",
                    "删除收藏夹失败",
                    AppError::bad_request("删除收藏夹失败"),
                )?;

            // 删除收藏夹照片
            CollectionPhotoMapper::delete_by_collection_id(txn, id, user_id).await?;

            AuditRecorder::append(
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
