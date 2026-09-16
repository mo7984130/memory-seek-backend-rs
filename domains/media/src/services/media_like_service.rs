use common::ext::ToOk;
use common::{Result, metrics_name, types::CursorPage, utils::MetricsTimerExt};
use types::{auth::user::UserId, cursor::TimeIdCursor, media::media::MediaId};

use crate::{
    mappers::media_like_mapper::MediaLikeMapper, repo::MediaLikeRepo,
    services::media_service::MediaService, state::MediaState,
};
use types::media::dto::media::MediaView;
use types::media::models::LikedMediasQuery;

pub(crate) struct MediaLikeService;

// 创建
impl MediaLikeService {
    /// 为媒体点赞.
    #[common_macros::metered(name = "like_media")]
    #[tracing::instrument(
        name = "like_media",
        skip_all,
        fields(user_id = %user_id, media_id = %media_id)
    )]
    pub async fn like(state: &MediaState, user_id: UserId, media_id: MediaId) -> Result<()> {
        MediaLikeRepo::like(state, user_id, media_id).await?;

        Ok(())
    }
}

// 查询
impl MediaLikeService {
    /// 查询用户点赞的媒体列表
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn get_user_liked_medias(
        state: &MediaState,
        user_id: UserId,
        req: LikedMediasQuery,
    ) -> Result<CursorPage<MediaView, TimeIdCursor<MediaId>>> {
        // 查询用户点赞的媒体ID列表和点赞时间
        let page = MediaLikeRepo::query_liked_media_ids(state, user_id, &req)
            .timed(metrics_name!("query_ids"))
            .await?;
        if page.records.is_empty() {
            return Ok(CursorPage::empty());
        }

        // 加载媒体详细信息
        let media_ids = page.records.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let medias = MediaService::load_medias_info(state, user_id, &media_ids)
            .timed(metrics_name!("load_medias_info"))
            .await?;

        page.replace_records(medias).to_ok()
    }
}

// 删除
impl MediaLikeService {
    /// 取消点赞.
    #[common_macros::metered(name = "unlike_media")]
    #[tracing::instrument(
        name = "unlike_media",
        skip_all,
        fields(user_id = %user_id, media_id = %media_id)
    )]
    pub async fn unlike(state: &MediaState, user_id: UserId, media_id: MediaId) -> Result<()> {
        MediaLikeRepo::unlike(state, user_id, media_id).await?;

        Ok(())
    }
}

// 媒体删除时
#[step_derive::declare_transaction_step(
    ctx = crate::services::media_service::MediaDeleteContext,
    slice = crate::services::media_service::MEDIA_DELETE_STEPS,
    name = "media_like_cleanup",
    owns = ["MediaLikeMapper"],
    method = on_media_delete,
)]
impl MediaLikeService {
    /// 清理媒体删除后失效的点赞记录.
    async fn on_media_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::media_service::MediaDeleteContext,
    ) -> common::error::contextual::Result<()> {
        let media_ids = ctx.media_ids();
        MediaLikeMapper::delete_all_by_media_ids(txn, &media_ids).await?;
        Ok(())
    }
}
