use crate::{
    mappers::{
        collection_mapper::CollectionMapper, collection_photo_mapper::CollectionPhotoMapper,
        photo_mapper::PhotoMapper,
    },
    services::photo_service::PhotoService,
    state::PhotoState,
};
use common::{
    Result,
    ext::OkExt,
    metrics_group, metrics_name, metrics_success,
    models::{CursorPage, TimeIdCursor},
    utils::{DbUtils, MetricsTimerExt},
};
use types::{
    auth::user::UserId,
    photo::{
        collection::CollectionId,
        dto::collection::{
            CollectionBriefView, CollectionPhotoAddBatchResult, CollectionPhotoRemoveBatchResult,
        },
        dto::photo::PhotoView,
        models::PhotoIds,
        photo::PhotoId,
    },
};

pub(crate) struct CollectionPhotoService;

// 查询
impl CollectionPhotoService {
    /// 获取包含指定照片的所有收藏夹
    #[tracing::instrument(skip_all)]
    pub async fn get_collections_by_photo(
        state: &PhotoState,
        user_id: UserId,
        photo_id: PhotoId,
    ) -> Result<Vec<CollectionBriefView>> {
        metrics_group!();

        let collection_ids =
            CollectionPhotoMapper::query_collection_ids_by_photo_id(&state.db, user_id, photo_id)
                .await?;

        if collection_ids.is_empty() {
            metrics_success!();
            return Ok(vec![]);
        }

        let collections = CollectionMapper::query_by_ids(&state.db, &collection_ids)
            .await?
            .into_iter()
            .map(CollectionBriefView::from)
            .collect();

        metrics_success!();
        Ok(collections)
    }

    #[tracing::instrument(name = "get_collection_photos", skip_all)]
    pub async fn get_photos(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        cursor: Option<TimeIdCursor<PhotoId>>,
        size: u64,
    ) -> Result<CursorPage<PhotoView, String>> {
        metrics_group!();

        let photo_ids = CollectionPhotoMapper::query_photo_id_by_collection_id(
            &state.db,
            user_id,
            collection_id,
            cursor.as_ref(),
            size + 1,
        )
        .timed(metrics_name!("query_photo_ids"))
        .await?;

        let CursorPage {
            records: photo_ids,
            has_more,
            ..
        } = CursorPage::from_oversize(photo_ids, size);

        let photo_vos = PhotoService::load_photos_info(state, user_id, &photo_ids)
            .timed(metrics_name!("load_photos_info"))
            .await?;
        let next_cursor = photo_vos.last().and_then(|vo| {
            PhotoId::parse_from_str_or_none(&vo.id).map(|id| {
                TimeIdCursor {
                    created_at: vo.created_at,
                    id,
                }
                .encode()
            })
        });

        metrics_success!();
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
    #[tracing::instrument(name = "add_collection_photos", skip_all)]
    pub async fn add_photos(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: PhotoIds,
    ) -> Result<CollectionPhotoAddBatchResult> {
        metrics_group!();

        // 插入前, 需要鉴权
        CollectionMapper::ensure_belong(&state.db, user_id, collection_id)
            .timed(metrics_name!("auth_check"))
            .await?;

        // 插入
        let photo_count = DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                let new_photo_count =
                    CollectionMapper::add_photos_batch(txn, collection_id, &photo_ids).await?;

                // 将新添加的第一张照片设为封面
                if let Some(photo_id) = photo_ids.first() {
                    let file_id = PhotoMapper::query_file_id_by_id(txn, *photo_id).await?;
                    CollectionMapper::update_cover_photo(
                        txn,
                        collection_id,
                        Some(*photo_id),
                        file_id,
                    )
                    .await?;
                }

                Ok(new_photo_count)
            })
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        metrics_success!();
        Ok(CollectionPhotoAddBatchResult {
            new_photo_count: photo_count,
        })
    }
}

// 删除
impl CollectionPhotoService {
    #[tracing::instrument(name = "remove_collection_photos", skip_all)]
    pub async fn remove_photos(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: PhotoIds,
    ) -> Result<CollectionPhotoRemoveBatchResult> {
        metrics_group!();

        let remove_count = DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                let collection =
                    CollectionMapper::ensure_belong_with_return(txn, user_id, collection_id)
                        .await?;

                // 先检查封面是否需要更新
                let need_update_cover = collection
                    .cover_photo_id
                    .map(|cover_pid| photo_ids.iter().any(|pid| pid.0 == cover_pid))
                    .unwrap_or(false);

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
                    let remaining_photo_ids =
                        CollectionPhotoMapper::query_photo_id_by_collection_id(
                            txn,
                            user_id,
                            collection_id,
                            None,
                            1,
                        )
                        .await?;

                    if let Some(photo_id) = remaining_photo_ids.first() {
                        let file_id = PhotoMapper::query_file_id_by_id(txn, *photo_id).await?;

                        CollectionMapper::update_cover_photo(
                            txn,
                            collection_id,
                            Some(*photo_id),
                            file_id,
                        )
                        .await?;
                    }
                }

                // 更新收藏夹照片数量
                CollectionMapper::update_photo_count_delta(txn, collection_id, -(rows as i64))
                    .await?;

                Ok(rows)
            })
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        metrics_success!();
        CollectionPhotoRemoveBatchResult {
            removed_photo_count: remove_count,
        }
        .to_ok()
    }
}
