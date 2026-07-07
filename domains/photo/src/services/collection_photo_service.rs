use crate::{
    mappers::{
        collection_mapper::CollectionMapper, collection_photo_mapper::CollectionPhotoMapper,
        photo_mapper::PhotoMapper,
    },
    models::{
        collection::{
            CollectionPhotoAddBatchResult, CollectionPhotoCursor, CollectionPhotoRemoveBatchResult,
            PhotoCollectionResult,
        },
        photo::PhotoResult,
    },
    services::photo_service::PhotoService,
    state::PhotoState,
};
use common::{
    Result,
    error::AppError,
    ext::{OkExt, ToErr, log_warn},
    metrics_group, metrics_success, metrics_timer_name,
    models::CursorPage,
    timed,
    utils::{DbUtils, MetricsTimerExt},
};
use entities::{
    auth::user::UserId,
    photo::{collection::CollectionId, photo::PhotoId},
};

pub(crate) struct CollectionPhotoService;

// 查询
impl CollectionPhotoService {
    /// 获取包含指定照片的所有收藏夹
    pub async fn get_collections_by_photo(
        state: &PhotoState,
        user_id: UserId,
        photo_id: PhotoId,
    ) -> Result<Vec<PhotoCollectionResult>> {
        metrics_group!("get_collections_by_photo");

        let collection_ids =
            CollectionPhotoMapper::query_collection_ids_by_photo_id(&state.db, user_id, photo_id)
                .await?;

        if collection_ids.is_empty() {
            metrics_success!("get_collections_by_photo");
            return Ok(vec![]);
        }

        let collections = CollectionMapper::query_by_ids(&state.db, &collection_ids).await?;
        let result: Vec<PhotoCollectionResult> =
            collections.into_iter().map(PhotoCollectionResult::from).collect();

        metrics_success!("get_collections_by_photo");
        Ok(result)
    }

    pub async fn get_photos(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        cursor: Option<String>,
        size: u64,
    ) -> Result<CursorPage<PhotoResult, String>> {
        metrics_group!("get_collection_photos");

        let decoded_cursor = cursor
            .as_ref()
            .and_then(|s| CollectionPhotoCursor::decode(s));
        let photo_ids = CollectionPhotoMapper::query_photo_id_by_collection_id(
            &state.db,
            user_id,
            collection_id,
            decoded_cursor.as_ref(),
            size + 1,
        )
        .timed(metrics_timer_name!(
            "get_collection_photos",
            "query_photo_ids"
        ))
        .await?;

        let CursorPage {
            records: photo_ids,
            has_more,
            ..
        } = CursorPage::from_oversize(photo_ids, size);

        let photo_vos = PhotoService::load_photos_info(state, user_id, &photo_ids)
            .timed(metrics_timer_name!(
                "get_collection_photos",
                "load_photos_info"
            ))
            .await?;
        let next_cursor = photo_vos.last().and_then(|vo| {
            PhotoId::parse_from_str_or_none(&vo.id).map(|id| {
                CollectionPhotoCursor {
                    created_at: vo.created_at,
                    id,
                }
                .encode()
            })
        });

        metrics_success!("get_collection_photos");
        CursorPage {
            records: photo_vos,
            has_more,
            next_cursor,
        }
        .to_ok()
    }
}

// 添加
impl CollectionPhotoService {
    pub async fn add_photos(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: Vec<PhotoId>,
    ) -> Result<CollectionPhotoAddBatchResult> {
        metrics_group!("add_collection_photos");

        if photo_ids.is_empty() {
            metrics_success!("add_collection_photos");
            return Ok(CollectionPhotoAddBatchResult::default());
        }

        // 插入前, 需要鉴权
        if !CollectionMapper::is_belong(&state.db, user_id, collection_id)
            .timed(metrics_timer_name!("add_collection_photos", "auth_check"))
            .await?
        {
            return log_warn(
                "collection_not_belong_user",
                "用户尝试添加照片到不是用户的收藏夹",
                "",
                AppError::forbidden("该收藏夹不属于你"),
            )
            .to_err();
        }

        // 插入
        let photo_count = timed!("add_collection_photos", "db_transaction", {
            DbUtils::write(&state.db, |txn| {
                Box::pin(async move {
                    CollectionPhotoMapper::inserts(txn, user_id, collection_id, &photo_ids).await?;
                    let new_photo_count =
                        CollectionPhotoMapper::count_photo_by_collection_id(txn, collection_id)
                            .await?;
                    CollectionMapper::update_photo_count(txn, collection_id, new_photo_count)
                        .await?;

                    // 将新添加的第一张照片设为封面
                    if let Some(photo_id) = photo_ids.first() {
                        let photos = PhotoMapper::query_by_ids(txn, &[*photo_id]).await?;
                        if let Some(photo) = photos.first() {
                            CollectionMapper::update_cover_file_id(
                                txn,
                                collection_id,
                                Some(photo.file_id.clone()),
                            )
                            .await?;
                        }
                    }

                    Ok(new_photo_count)
                })
            })
            .await
        })?;

        metrics_success!("add_collection_photos");
        Ok(CollectionPhotoAddBatchResult {
            new_photo_count: photo_count,
        })
    }
}

// 删除
impl CollectionPhotoService {
    pub async fn remove_photos(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: Vec<PhotoId>,
    ) -> Result<CollectionPhotoRemoveBatchResult> {
        metrics_group!("remove_collection_photos");

        if photo_ids.is_empty() {
            metrics_success!("remove_collection_photos");
            return Ok(CollectionPhotoRemoveBatchResult::default());
        }

        // 移除时鉴权 使用user_id
        let remove_count = timed!("remove_collection_photos", "db_transaction", {
            DbUtils::write(&state.db, |txn| {
                Box::pin(async move {
                    // 先检查封面是否需要更新
                    let collection = CollectionMapper::query_by_id(txn, collection_id).await?;
                    let need_update_cover = if let Some(col) = &collection {
                        if let Some(cover_file_id) = &col.cover_file_id {
                            // 检查被删除的照片中是否有封面照片
                            let deleted_photos =
                                PhotoMapper::query_by_ids(txn, &photo_ids).await?;
                            deleted_photos
                                .iter()
                                .any(|p| &p.file_id == cover_file_id)
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    let rows = CollectionPhotoMapper::delete_by_collection_id_and_photo_ids(
                        txn,
                        user_id,
                        collection_id,
                        &photo_ids,
                    )
                    .await?;

                    // 如果封面照片被删除，更新封面
                    if need_update_cover {
                        // 获取剩余的第一张照片作为新封面
                        let remaining_photo_ids = CollectionPhotoMapper::query_photo_id_by_collection_id(
                            txn,
                            user_id,
                            collection_id,
                            None,
                            1,
                        )
                        .await?;

                        let new_cover_file_id = if let Some(photo_id) = remaining_photo_ids.first()
                        {
                            let photos = PhotoMapper::query_by_ids(txn, &[*photo_id]).await?;
                            photos.first().map(|p| p.file_id.clone())
                        } else {
                            None
                        };

                        CollectionMapper::update_cover_file_id(
                            txn,
                            collection_id,
                            new_cover_file_id,
                        )
                        .await?;
                    }

                    // 更新收藏夹照片数量
                    let new_photo_count =
                        CollectionPhotoMapper::count_photo_by_collection_id(txn, collection_id)
                            .await?;
                    CollectionMapper::update_photo_count(txn, collection_id, new_photo_count)
                        .await?;

                    Ok(rows)
                })
            })
            .await
        })?;

        metrics_success!("remove_collection_photos");
        CollectionPhotoRemoveBatchResult {
            removed_photo_count: remove_count,
        }
        .to_ok()
    }
}
