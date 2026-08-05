use std::collections::HashMap;

use common::{
    Result,
    error::AppError,
    ext::{ToErr, log_warn},
    metrics_group, metrics_name, metrics_success,
    models::CursorPage,
    timed,
    utils::{DbUtils, MetricsTimerExt},
};
use sea_orm::entity::prelude::DateTimeUtc;
use types::{auth::user::UserId, cursor::TimeIdCursor, photo::photo::PhotoId};

use crate::{
    mappers::{photo_like_mapper::PhotoLikeMapper, photo_mapper::PhotoMapper},
    services::photo_service::PhotoService,
    state::PhotoState,
};
use types::photo::dto::photo::PhotoView;
use types::photo::models::LikedPhotosQuery;

pub(crate) struct PhotoLikeService;

// 创建
impl PhotoLikeService {
    #[tracing::instrument(name = "like_photo", skip_all)]
    pub async fn like(state: &PhotoState, user_id: UserId, photo_id: PhotoId) -> Result<()> {
        metrics_group!();

        timed!("db_transaction", {
            DbUtils::write(&state.db, |txn| {
                Box::pin(async move {
                    PhotoMapper::ensure_exist(txn, photo_id).await?;

                    let inserted = PhotoLikeMapper::insert(txn, user_id, photo_id).await?;

                    if !inserted {
                        return log_warn(
                            "photo_like_already_exist",
                            "用户尝试点赞一个已经点赞过的照片",
                            AppError::bad_request("已经点赞过"),
                        )
                        .to_err();
                    }

                    // 增加点赞总数
                    PhotoMapper::update_like_count_delta(txn, photo_id, 1).await?;
                    Ok(())
                })
            })
            .await
        })?;

        metrics_success!();
        Ok(())
    }
}

// 查询
impl PhotoLikeService {
    /// 查询用户点赞的照片列表（带分页和照片详情）
    #[tracing::instrument(skip_all)]
    pub async fn get_user_liked_photos(
        state: &PhotoState,
        user_id: UserId,
        param: LikedPhotosQuery,
    ) -> Result<CursorPage<PhotoView, String>> {
        metrics_group!();

        // 查询用户点赞的照片ID列表和点赞时间（多查一个用于判断 has_more）
        let photo_ids_with_like_time = PhotoLikeMapper::query_user_liked_photo_ids(
            &state.db,
            user_id,
            &param.cursor,
            param.size + 1,
        )
        .timed(metrics_name!("query_ids"))
        .await?;

        // 构建 CursorPage（只提取 photo_id 用于分页判断）
        let photo_ids: Vec<PhotoId> = photo_ids_with_like_time.iter().map(|(id, _)| *id).collect();
        let CursorPage {
            records: photo_ids,
            has_more,
            ..
        } = CursorPage::from_oversize(photo_ids, param.size);

        if photo_ids.is_empty() {
            metrics_success!();
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
                let id = PhotoId::parse_from_str_or_none(&p.id)?;
                let like_created_at = like_time_map.get(&id).copied()?;
                Some(
                    TimeIdCursor {
                        created_at: like_created_at,
                        id,
                    }
                    .encode(),
                )
            })
        } else {
            None
        };

        metrics_success!();
        Ok(CursorPage {
            records: photos,
            next_cursor,
            has_more,
        })
    }
}

// 删除
impl PhotoLikeService {
    #[tracing::instrument(name = "unlike_photo", skip_all)]
    pub async fn unlike(state: &PhotoState, user_id: UserId, photo_id: PhotoId) -> Result<()> {
        metrics_group!();

        timed!("db_transaction", {
            DbUtils::write(&state.db, |txn| {
                Box::pin(async move {
                    let deleted = PhotoLikeMapper::delete(txn, user_id, photo_id).await?;

                    if !deleted {
                        return log_warn(
                            "photo_like_not_exist",
                            "用户尝试取消点赞一个未点赞过的照片",
                            AppError::bad_request("还未点赞"),
                        )
                        .to_err();
                    }

                    // 减少点赞总数
                    PhotoMapper::update_like_count_delta(txn, photo_id, -1).await?;
                    Ok(())
                })
            })
            .await
        })?;

        metrics_success!();
        Ok(())
    }
}
