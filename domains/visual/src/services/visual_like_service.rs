use common::ext::ToOk;
use common::{Result, metrics_name, types::CursorPage, utils::MetricsTimerExt};
use types::{auth::user::UserId, cursor::TimeIdCursor, visual::visual::VisualId};

use crate::{
    mappers::visual_like_mapper::VisualLikeMapper, repo::VisualLikeRepo,
    services::visual_service::VisualService, state::VisualState,
};
use types::visual::dto::visual::VisualView;
use types::visual::models::LikedVisualsQuery;

pub(crate) struct VisualLikeService;

// 创建
impl VisualLikeService {
    /// 为影像点赞.
    #[common_macros::metered(name = "like_visual")]
    #[tracing::instrument(
        name = "like_visual",
        skip_all,
        fields(user_id = %user_id, visual_id = %visual_id)
    )]
    pub async fn like(state: &VisualState, user_id: UserId, visual_id: VisualId) -> Result<()> {
        VisualLikeRepo::like(state, user_id, visual_id).await?;

        Ok(())
    }
}

// 查询
impl VisualLikeService {
    /// 查询用户点赞的影像列表
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn get_user_liked_visuals(
        state: &VisualState,
        user_id: UserId,
        req: LikedVisualsQuery,
    ) -> Result<CursorPage<VisualView, TimeIdCursor<VisualId>>> {
        // 查询用户点赞的影像ID列表和点赞时间
        let page = VisualLikeRepo::query_liked_visual_ids(state, user_id, &req)
            .timed(metrics_name!("query_ids"))
            .await?;
        if page.records.is_empty() {
            return Ok(CursorPage::empty());
        }

        // 加载影像详细信息
        let visual_ids = page.records.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let visuals = VisualService::load_visuals_info(state, user_id, &visual_ids)
            .timed(metrics_name!("load_visuals_info"))
            .await?;

        page.replace_records(visuals).to_ok()
    }
}

// 删除
impl VisualLikeService {
    /// 取消点赞.
    #[common_macros::metered(name = "unlike_visual")]
    #[tracing::instrument(
        name = "unlike_visual",
        skip_all,
        fields(user_id = %user_id, visual_id = %visual_id)
    )]
    pub async fn unlike(state: &VisualState, user_id: UserId, visual_id: VisualId) -> Result<()> {
        VisualLikeRepo::unlike(state, user_id, visual_id).await?;

        Ok(())
    }
}

// 影像删除时
#[step_derive::declare_transaction_step(
    ctx = crate::services::visual_service::VisualDeleteContext,
    slice = crate::services::visual_service::MEDIA_DELETE_STEPS,
    name = "visual_like_cleanup",
    owns = ["VisualLikeMapper"],
    method = on_visual_delete,
)]
impl VisualLikeService {
    /// 清理影像删除后失效的点赞记录.
    async fn on_visual_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::visual_service::VisualDeleteContext,
    ) -> common::error::contextual::Result<()> {
        let visual_ids = ctx.visual_ids();
        VisualLikeMapper::delete_all_by_visual_ids(txn, &visual_ids).await?;
        Ok(())
    }
}
