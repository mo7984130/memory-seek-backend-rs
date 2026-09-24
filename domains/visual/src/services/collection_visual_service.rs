use crate::repo::CollectionRepo;
use crate::{services::visual_service::VisualService, state::VisualState};
use common::{Result, ext::ToOk, metrics_name, types::CursorPage, utils::MetricsTimerExt};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    visual::{
        collection::CollectionId,
        dto::collection::{
            CollectionBriefView, CollectionVisualAddBatchResult, CollectionVisualCursorPageParam,
            CollectionVisualRemoveBatchResult,
        },
        dto::visual::VisualView,
        models::VisualIds,
        visual::VisualId,
    },
};

pub(crate) struct CollectionVisualService;

// 查询
impl CollectionVisualService {
    /// 获取包含指定影像的所有收藏夹
    #[common_macros::metered]
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, visual_id = %visual_id)
    )]
    pub async fn get_collections_by_visual(
        state: &VisualState,
        user_id: UserId,
        visual_id: VisualId,
    ) -> Result<Vec<CollectionBriefView>> {
        let collection_ids =
            CollectionRepo::query_collection_ids_by_visual(state, user_id, visual_id).await?;

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

    /// 按游标查询相册中的影像, 并补充影像视图信息.
    #[common_macros::metered(name = "get_collection_visuals")]
    #[tracing::instrument(
        name = "get_collection_visuals",
        skip_all,
        fields(user_id = %user_id, collection_id = %collection_id)
    )]
    pub async fn get_visuals(
        state: &VisualState,
        user_id: UserId,
        collection_id: CollectionId,
        req: CollectionVisualCursorPageParam,
    ) -> Result<CursorPage<VisualView, TimeIdCursor<VisualId>>> {
        let page = CollectionRepo::query_collection_visual_ids(state, user_id, collection_id, &req)
            .timed(metrics_name!("query_visual_ids"))
            .await?;

        let visual_vos = VisualService::load_visuals_info(state, user_id, &page.records)
            .timed(metrics_name!("load_visuals_info"))
            .await?;
        Ok(page
            .replace_records(visual_vos)
            .with_next_cursor(|vo| TimeIdCursor {
                time_at: vo.created_at,
                id: vo.id,
            }))
    }
}

// 添加
impl CollectionVisualService {
    /// 批量将影像加入相册, 并返回实际新增数量.
    #[common_macros::metered(name = "add_collection_visuals")]
    #[tracing::instrument(
        name = "add_collection_visuals",
        skip_all,
        fields(user_id = %user_id, collection_id = %collection_id, count = %visual_ids.len())
    )]
    pub async fn add_visuals(
        state: &VisualState,
        user_id: UserId,
        collection_id: CollectionId,
        visual_ids: VisualIds,
    ) -> Result<CollectionVisualAddBatchResult> {
        // 确认归属
        CollectionRepo::ensure_belong(state, user_id, collection_id).await?;

        // 添加影像
        let visual_count =
            CollectionRepo::add_collection_visuals(state, user_id, collection_id, &visual_ids)
                .await?;

        Ok(CollectionVisualAddBatchResult {
            new_visual_count: visual_count,
        })
    }
}

// 删除
impl CollectionVisualService {
    /// 移除收藏夹影像.
    #[common_macros::metered(name = "remove_collection_visuals")]
    #[tracing::instrument(
        name = "remove_collection_visuals",
        skip_all,
        fields(user_id = %user_id, collection_id = %collection_id, count = %visual_ids.len())
    )]
    pub async fn remove_visuals(
        state: &VisualState,
        user_id: UserId,
        collection_id: CollectionId,
        visual_ids: VisualIds,
    ) -> Result<CollectionVisualRemoveBatchResult> {
        // 校验归属
        let collection =
            CollectionRepo::ensure_belong_with_return(state, user_id, collection_id).await?;

        // 移除影像
        let remove_count =
            CollectionRepo::remove_collection_visuals(state, user_id, collection, &visual_ids)
                .await?;

        CollectionVisualRemoveBatchResult {
            removed_visual_count: remove_count,
        }
        .to_ok()
    }
}
