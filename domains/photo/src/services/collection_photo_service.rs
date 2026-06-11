use crate::{
    mappers::{
        collection_mapper::CollectionMapper, collection_photo_mapper::CollectionPhotoMapper,
    },
    models::{
        collection::{
            CollectionPhotoAddBatchResult, CollectionPhotoCursor, CollectionPhotoRemoveBatchResult,
        },
        photo::PhotoVO,
    },
    services::photo_service::PhotoService,
    state::PhotoState,
};
use common::{
    Result,
    error::AppError,
    ext::{OkExt, ToErr, log_warn},
    models::CursorPage,
    utils::DbUtils,
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
    ) -> Result<CursorPage<PhotoVO, String>> {
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
        .await?;

        let CursorPage {
            records: photo_ids,
            has_more,
            ..
        } = CursorPage::from_oversize(photo_ids, size);

        let photo_vos = PhotoService::load_photos_info(state, user_id, &photo_ids).await?;
        let next_cursor = photo_vos.last().and_then(|vo| {
            PhotoId::parse_from_str_or_none(&vo.id).and_then(|id| {
                Some(
                    CollectionPhotoCursor {
                        created_at: vo.created_at,
                        id,
                    }
                    .encode(),
                )
            })
        });

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
        if photo_ids.is_empty() {
            return Ok(CollectionPhotoAddBatchResult::default());
        }

        // 插入前, 需要鉴权
        if !CollectionPhotoMapper::is_belong(&state.db, user_id, collection_id).await? {
            return log_warn(
                "collection_not_belong_user",
                "用户尝试添加照片到不是用户的收藏夹",
                "",
                AppError::forbidden("该收藏夹不属于你"),
            )
            .to_err();
        }

        // 插入
        let photo_count = DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                CollectionPhotoMapper::inserts(txn, user_id, collection_id, &photo_ids).await?;
                let new_photo_count =
                    CollectionPhotoMapper::count_photo_by_collection_id(txn, collection_id).await?;
                CollectionMapper::update_photo_count(txn, collection_id, new_photo_count).await?;

                Ok(new_photo_count)
            })
        })
        .await?;

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
        if photo_ids.is_empty() {
            return Ok(CollectionPhotoRemoveBatchResult::default());
        }

        // 移除时鉴权 使用user_id
        let remove_count = DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                let rows = CollectionPhotoMapper::delete_by_collection_id_and_photo_ids(
                    txn,
                    user_id,
                    collection_id,
                    &photo_ids,
                )
                .await?;
                if rows == 0 {
                    return log_warn(
                        "delete_collection_photos_row_zero",
                        "删除收藏夹照片的印象行数为零",
                        "",
                        AppError::bad_request("删除失败"),
                    )
                    .to_err();
                }

                Ok(rows)
            })
        })
        .await?;

        CollectionPhotoRemoveBatchResult {
            removed_photo_count: remove_count,
        }
        .to_ok()
    }
}
