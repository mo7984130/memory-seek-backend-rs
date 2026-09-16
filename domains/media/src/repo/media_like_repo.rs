use audit::{AuditEvent, AuditRecorder};
use common::{
    db_transaction,
    error::{AppError, ContextualError, contextual::Result},
    metrics_name,
    time::DateTime,
    types::CursorPage,
    utils::MetricsTimerExt,
};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    media::{models::LikedMediasQuery, media::MediaId},
};

use crate::state::MediaState;
use crate::{
    mappers::{media_like_mapper::MediaLikeMapper, media_mapper::MediaMapper},
    repo::MediaRepo,
};

pub(crate) struct MediaLikeRepo;

impl MediaLikeRepo {
    /// 点赞媒体.
    pub(crate) async fn like(state: &MediaState, user_id: UserId, media_id: MediaId) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            // 确认存在
            MediaMapper::ensure_exist(txn, media_id).await?;
            // 插入
            if !MediaLikeMapper::insert(txn, user_id, media_id).await? {
                return Err(ContextualError::error_without_source(
                    "media_already_liked",
                    "媒体已经点赞过",
                    AppError::bad_request("已经点赞过"),
                ));
            }
            // 更新计数
            MediaMapper::update_like_count_delta(txn, media_id, 1).await?;

            AuditRecorder::append(
                txn,
                AuditEvent::new("like")
                    .with_actor(user_id.0)
                    .with_target("media", media_id.0),
            )
            .await?;
            Ok(())
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        MediaRepo::cache_media_like_status(state, user_id, media_id, true).await;
        MediaRepo::invalidate_media_info(state, media_id).await;

        Ok(())
    }

    /// 取消点赞
    pub(crate) async fn unlike(
        state: &MediaState,
        user_id: UserId,
        media_id: MediaId,
    ) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            if !MediaLikeMapper::delete(txn, user_id, media_id).await? {
                return Err(ContextualError::error_without_source(
                    "media_not_liked",
                    "媒体尚未点赞",
                    AppError::bad_request("还未点赞"),
                ));
            }
            MediaMapper::update_like_count_delta(txn, media_id, -1).await?;
            AuditRecorder::append(
                txn,
                AuditEvent::new("unlike")
                    .with_actor(user_id.0)
                    .with_target("media", media_id.0),
            )
            .await?;
            Ok(())
        })
        .timed(metrics_name!("db_transaction"))
        .await?;
        MediaRepo::cache_media_like_status(state, user_id, media_id, false).await;
        MediaRepo::invalidate_media_info(state, media_id).await;
        Ok(())
    }

    /// 查询用户点赞过的媒体 ID.
    pub(crate) async fn query_liked_media_ids(
        state: &MediaState,
        user_id: UserId,
        req: &LikedMediasQuery,
    ) -> Result<CursorPage<(MediaId, DateTime), TimeIdCursor<MediaId>>> {
        MediaLikeMapper::query_user_liked_media_ids(&state.db, user_id, &req.cursor, req.size)
            .timed(metrics_name!("query_ids"))
            .await
    }
}
