use std::collections::HashMap;

use common::{Result, metrics_name, models::CursorPage, utils::MetricsTimerExt};
use sea_orm::entity::prelude::DateTimeUtc;
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
    ) -> Result<CursorPage<PhotoView, String>> {
        // 查询用户点赞的照片ID列表和点赞时间（mapper 内部多查 1 条用于判断 has_more）
        let photo_ids_with_like_time = state
            .repo
            .query_liked_photo_ids(user_id, &req)
            .timed(metrics_name!("query_ids"))
            .await?;

        // 构建 CursorPage（只提取 photo_id 用于分页判断）
        let photo_ids: Vec<PhotoId> = photo_ids_with_like_time.iter().map(|(id, _)| *id).collect();
        let CursorPage {
            records: photo_ids,
            has_more,
            ..
        } = CursorPage::from_oversize(photo_ids, req.size);

        if photo_ids.is_empty() {
            return Ok(CursorPage::empty());
        }

        // 构建 photo_id -> like_created_at 的映射
        let like_time_map: HashMap<PhotoId, DateTimeUtc> = photo_ids_with_like_time
            .into_iter()
            .take(photo_ids.len())
            .collect();

        // 加载照片详细信息
        let photos = PhotoService::load_photos_info(state, user_id, &photo_ids).await?;

        // 生成 next_cursor（使用点赞时间而非照片上传时间）
        let next_cursor = if has_more {
            photos.last().and_then(|p| {
                let like_created_at = like_time_map.get(&p.id).copied()?;
                Some(
                    TimeIdCursor {
                        created_at: like_created_at,
                        id: p.id,
                    }
                    .encode(),
                )
            })
        } else {
            None
        };

        Ok(CursorPage {
            records: photos,
            next_cursor,
            has_more,
        })
    }
}

// 删除
impl PhotoLikeService {
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
#[step_derive::declare_step(
    ctx = crate::repo::photo_repo::PhotoDeleteContext,
    slice = crate::repo::photo_repo::PHOTO_DELETE_STEPS,
    name = "photo_like_cleanup",
    owns = ["PhotoLikeMapper"],
)]
impl PhotoLikeService {
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
