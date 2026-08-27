use common::ext::ToOk;
use common::{Result, metrics_name, types::CursorPage, utils::MetricsTimerExt};
use types::{auth::user::UserId, cursor::TimeIdCursor, photo::photo::PhotoId};

use crate::{
    mappers::photo_like_mapper::PhotoLikeMapper, repo::PhotoLikeRepo,
    services::photo_service::PhotoService, state::PhotoState,
};
use types::photo::dto::photo::PhotoView;
use types::photo::models::LikedPhotosQuery;

pub(crate) struct PhotoLikeService;

// 创建
impl PhotoLikeService {
    /// 为照片点赞.
    #[common_macros::metered(name = "like_photo")]
    #[tracing::instrument(
        name = "like_photo",
        skip_all,
        fields(user_id = %user_id, photo_id = %photo_id)
    )]
    pub async fn like(state: &PhotoState, user_id: UserId, photo_id: PhotoId) -> Result<()> {
        PhotoLikeRepo::like(state, user_id, photo_id).await?;

        Ok(())
    }
}

// 查询
impl PhotoLikeService {
    /// 查询用户点赞的照片列表
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn get_user_liked_photos(
        state: &PhotoState,
        user_id: UserId,
        req: LikedPhotosQuery,
    ) -> Result<CursorPage<PhotoView, TimeIdCursor<PhotoId>>> {
        // 查询用户点赞的照片ID列表和点赞时间
        let page = PhotoLikeRepo::query_liked_photo_ids(state, user_id, &req)
            .timed(metrics_name!("query_ids"))
            .await?;
        if page.records.is_empty() {
            return Ok(CursorPage::empty());
        }

        // 加载照片详细信息
        let photo_ids = page.records.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let photos = PhotoService::load_photos_info(state, user_id, &photo_ids).await?;

        page.replace_records(photos).to_ok()
    }
}

// 删除
impl PhotoLikeService {
    /// 取消点赞.
    #[common_macros::metered(name = "unlike_photo")]
    #[tracing::instrument(
        name = "unlike_photo",
        skip_all,
        fields(user_id = %user_id, photo_id = %photo_id)
    )]
    pub async fn unlike(state: &PhotoState, user_id: UserId, photo_id: PhotoId) -> Result<()> {
        PhotoLikeRepo::unlike(state, user_id, photo_id).await?;

        Ok(())
    }
}

// 照片删除时
#[step_derive::declare_transaction_step(
    ctx = crate::services::photo_service::PhotoDeleteContext,
    slice = crate::services::photo_service::PHOTO_DELETE_STEPS,
    name = "photo_like_cleanup",
    owns = ["PhotoLikeMapper"],
)]
impl PhotoLikeService {
    /// 清理照片删除后失效的点赞记录.
    async fn on_photo_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::photo_service::PhotoDeleteContext,
    ) -> common::error::contextual::Result<()> {
        let photo_ids = ctx.photo_ids();
        PhotoLikeMapper::delete_all_by_photo_ids(txn, &photo_ids).await?;
        Ok(())
    }
}
