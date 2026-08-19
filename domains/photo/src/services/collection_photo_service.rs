use crate::repo::CollectionRepo;
use crate::{services::photo_service::PhotoService, state::PhotoState};
use common::{Result, ext::OkExt, metrics_name, models::CursorPage, utils::MetricsTimerExt};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    photo::{
        collection::CollectionId,
        dto::collection::{
            CollectionBriefView, CollectionPhotoAddBatchResult, CollectionPhotoCursorPageParam,
            CollectionPhotoRemoveBatchResult,
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
    #[common::metered]
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, photo_id = %photo_id)
    )]
    pub async fn get_collections_by_photo(
        state: &PhotoState,
        user_id: UserId,
        photo_id: PhotoId,
    ) -> Result<Vec<CollectionBriefView>> {
        let collection_ids =
            CollectionRepo::query_collection_ids_by_photo(state, user_id, photo_id).await?;

        if collection_ids.is_empty() {
            return Ok(vec![]);
        }

        let collections = CollectionRepo::query_collection_briefs(state, &collection_ids)
            .await?
            .into_iter()
            .map(|(id, name)| CollectionBriefView { id, name })
            .collect();

        Ok(collections)
    }

    /// 按游标查询相册中的照片, 并补充照片视图信息.
    #[common::metered(name = "get_collection_photos")]
    #[tracing::instrument(
        name = "get_collection_photos",
        skip_all,
        fields(user_id = %user_id, collection_id = %collection_id)
    )]
    pub async fn get_photos(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        req: CollectionPhotoCursorPageParam,
    ) -> Result<CursorPage<PhotoView, TimeIdCursor<PhotoId>>> {
        let page = CollectionRepo::query_collection_photo_ids(state, user_id, collection_id, &req)
            .timed(metrics_name!("query_photo_ids"))
            .await?;

        let photo_vos = PhotoService::load_photos_info(state, user_id, &page.records)
            .timed(metrics_name!("load_photos_info"))
            .await?;
        Ok(page
            .replace_records(photo_vos)
            .with_next_cursor(|vo| TimeIdCursor {
                time_at: vo.created_at,
                id: vo.id,
            }))
    }
}

// 添加
impl CollectionPhotoService {
    /// 批量将照片加入相册, 并返回实际新增数量.
    #[common::metered(name = "add_collection_photos")]
    #[tracing::instrument(
        name = "add_collection_photos",
        skip_all,
        fields(user_id = %user_id, collection_id = %collection_id, count = %photo_ids.len())
    )]
    pub async fn add_photos(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: PhotoIds,
    ) -> Result<CollectionPhotoAddBatchResult> {
        let photo_count =
            CollectionRepo::add_collection_photos(state, user_id, collection_id, &photo_ids)
                .await?;

        Ok(CollectionPhotoAddBatchResult {
            new_photo_count: photo_count,
        })
    }
}

// 删除
impl CollectionPhotoService {
    /// 批量从相册移除照片, 并返回实际移除数量.
    #[common::metered(name = "remove_collection_photos")]
    #[tracing::instrument(
        name = "remove_collection_photos",
        skip_all,
        fields(user_id = %user_id, collection_id = %collection_id, count = %photo_ids.len())
    )]
    pub async fn remove_photos(
        state: &PhotoState,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: PhotoIds,
    ) -> Result<CollectionPhotoRemoveBatchResult> {
        let remove_count =
            CollectionRepo::remove_collection_photos(state, user_id, collection_id, &photo_ids)
                .await?;

        CollectionPhotoRemoveBatchResult {
            removed_photo_count: remove_count,
        }
        .to_ok()
    }
}
