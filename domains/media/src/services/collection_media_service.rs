use crate::repo::CollectionRepo;
use crate::{services::media_service::MediaService, state::MediaState};
use common::{Result, ext::ToOk, metrics_name, types::CursorPage, utils::MetricsTimerExt};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    media::{
        collection::CollectionId,
        dto::collection::{
            CollectionBriefView, CollectionMediaAddBatchResult, CollectionMediaCursorPageParam,
            CollectionMediaRemoveBatchResult,
        },
        dto::media::MediaView,
        models::MediaIds,
        media::MediaId,
    },
};

pub(crate) struct CollectionMediaService;

// 查询
impl CollectionMediaService {
    /// 获取包含指定照片的所有收藏夹
    #[common_macros::metered]
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, media_id = %media_id)
    )]
    pub async fn get_collections_by_media(
        state: &MediaState,
        user_id: UserId,
        media_id: MediaId,
    ) -> Result<Vec<CollectionBriefView>> {
        let collection_ids =
            CollectionRepo::query_collection_ids_by_media(state, user_id, media_id).await?;

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
    #[common_macros::metered(name = "get_collection_medias")]
    #[tracing::instrument(
        name = "get_collection_medias",
        skip_all,
        fields(user_id = %user_id, collection_id = %collection_id)
    )]
    pub async fn get_medias(
        state: &MediaState,
        user_id: UserId,
        collection_id: CollectionId,
        req: CollectionMediaCursorPageParam,
    ) -> Result<CursorPage<MediaView, TimeIdCursor<MediaId>>> {
        let page = CollectionRepo::query_collection_media_ids(state, user_id, collection_id, &req)
            .timed(metrics_name!("query_media_ids"))
            .await?;

        let media_vos = MediaService::load_medias_info(state, user_id, &page.records)
            .timed(metrics_name!("load_medias_info"))
            .await?;
        Ok(page
            .replace_records(media_vos)
            .with_next_cursor(|vo| TimeIdCursor {
                time_at: vo.created_at,
                id: vo.id,
            }))
    }
}

// 添加
impl CollectionMediaService {
    /// 批量将照片加入相册, 并返回实际新增数量.
    #[common_macros::metered(name = "add_collection_medias")]
    #[tracing::instrument(
        name = "add_collection_medias",
        skip_all,
        fields(user_id = %user_id, collection_id = %collection_id, count = %media_ids.len())
    )]
    pub async fn add_medias(
        state: &MediaState,
        user_id: UserId,
        collection_id: CollectionId,
        media_ids: MediaIds,
    ) -> Result<CollectionMediaAddBatchResult> {
        // 确认归属
        CollectionRepo::ensure_belong(state, user_id, collection_id).await?;

        // 添加照片
        let media_count =
            CollectionRepo::add_collection_medias(state, user_id, collection_id, &media_ids)
                .await?;

        Ok(CollectionMediaAddBatchResult {
            new_media_count: media_count,
        })
    }
}

// 删除
impl CollectionMediaService {
    /// 移除收藏夹照片.
    #[common_macros::metered(name = "remove_collection_medias")]
    #[tracing::instrument(
        name = "remove_collection_medias",
        skip_all,
        fields(user_id = %user_id, collection_id = %collection_id, count = %media_ids.len())
    )]
    pub async fn remove_medias(
        state: &MediaState,
        user_id: UserId,
        collection_id: CollectionId,
        media_ids: MediaIds,
    ) -> Result<CollectionMediaRemoveBatchResult> {
        // 校验归属
        let collection =
            CollectionRepo::ensure_belong_with_return(state, user_id, collection_id).await?;

        // 移除照片
        let remove_count =
            CollectionRepo::remove_collection_medias(state, user_id, collection, &media_ids)
                .await?;

        CollectionMediaRemoveBatchResult {
            removed_media_count: remove_count,
        }
        .to_ok()
    }
}
