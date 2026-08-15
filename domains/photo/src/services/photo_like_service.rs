use common::{Result, metrics_name, models::CursorPage, utils::MetricsTimerExt};
use types::{auth::user::UserId, cursor::TimeIdCursor, photo::photo::PhotoId};

use crate::{
    mappers::photo_like_mapper::PhotoLikeMapper, services::photo_service::PhotoService,
    state::PhotoState,
};
use types::photo::dto::photo::PhotoView;
use types::photo::models::LikedPhotosQuery;

pub(crate) struct PhotoLikeService;

// 创建
impl PhotoLikeService {
    /// 为照片点赞; 重复点赞由仓储层保证幂等.
    #[common::metered(name = "like_photo")]
    #[tracing::instrument(
        name = "like_photo",
        skip_all,
        fields(user_id = %user_id, photo_id = %photo_id)
    )]
    pub async fn like(state: &PhotoState, user_id: UserId, photo_id: PhotoId) -> Result<()> {
        state.repo.like_photo(user_id, photo_id).await?;

        Ok(())
    }
}

// 查询
impl PhotoLikeService {
    /// 查询用户点赞的照片列表（带分页和照片详情）
    #[common::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn get_user_liked_photos(
        state: &PhotoState,
        user_id: UserId,
        req: LikedPhotosQuery,
    ) -> Result<CursorPage<PhotoView, TimeIdCursor<PhotoId>>> {
        // 查询用户点赞的照片ID列表和点赞时间（mapper 内部多查 1 条用于判断 has_more）
        let page = state
            .repo
            .query_liked_photo_ids(user_id, &req)
            .timed(metrics_name!("query_ids"))
            .await?;

        let page =
            page.with_next_cursor(|&(id, created_at)| Ok(TimeIdCursor { id, created_at }))?;
        let photo_ids = page.records.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        if photo_ids.is_empty() {
            return Ok(CursorPage::empty());
        }

        // 加载照片详细信息
        let photos = PhotoService::load_photos_info(state, user_id, &photo_ids).await?;

        Ok(page.replace_records(photos))
    }
}

// 删除
impl PhotoLikeService {
    /// 取消用户对照片的点赞.
    #[common::metered(name = "unlike_photo")]
    #[tracing::instrument(
        name = "unlike_photo",
        skip_all,
        fields(user_id = %user_id, photo_id = %photo_id)
    )]
    pub async fn unlike(state: &PhotoState, user_id: UserId, photo_id: PhotoId) -> Result<()> {
        state.repo.unlike_photo(user_id, photo_id).await?;

        Ok(())
    }
}

// 照片删除步骤:照片点赞清理
#[step_derive::declare_transaction_step(
    ctx = crate::repo::photo_repo::PhotoDeleteContext,
    slice = crate::repo::photo_repo::PHOTO_DELETE_STEPS,
    name = "photo_like_cleanup",
    owns = ["PhotoLikeMapper"],
)]
impl PhotoLikeService {
    /// 清理照片删除后失效的点赞记录.
    async fn on_photo_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::repo::photo_repo::PhotoDeleteContext,
    ) -> common::Result<()> {
        let photo_ids = ctx.photo_ids();
        PhotoLikeMapper::delete_all_by_photo_ids(txn, &photo_ids).await?;
        Ok(())
    }
}
