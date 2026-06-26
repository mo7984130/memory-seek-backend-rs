use crate::{
    mappers::{
        collection_mapper::CollectionMapper, collection_photo_mapper::CollectionPhotoMapper,
    },
    models::{
        collection::{
            CollectionPhotoAddBatchResult, CollectionPhotoCursor, CollectionPhotoRemoveBatchResult,
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
    metrics_group, metrics_success, metrics_timer_name, timed,
    models::CursorPage,
    utils::{DbUtils, MetricsTimerExt},
};
use entities::{
    auth::user::UserId,
    photo::{collection::CollectionId, photo::PhotoId},
};

pub(crate) struct CollectionPhotoService;

// 查询
impl CollectionPhotoService {
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
        .timed(metrics_timer_name!("get_collection_photos", "query_photo_ids"))
        .await?;

        let CursorPage {
            records: photo_ids,
            has_more,
            ..
        } = CursorPage::from_oversize(photo_ids, size);

        let photo_vos = PhotoService::load_photos_info(state, user_id, &photo_ids)
            .timed(metrics_timer_name!("get_collection_photos", "load_photos_info"))
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
                    let rows = CollectionPhotoMapper::delete_by_collection_id_and_photo_ids(
                        txn,
                        user_id,
                        collection_id,
                        &photo_ids,
                    )
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
