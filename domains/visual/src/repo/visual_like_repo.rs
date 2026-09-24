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
    visual::{models::LikedVisualsQuery, visual::VisualId},
};

use crate::state::VisualState;
use crate::{
    mappers::{visual_like_mapper::VisualLikeMapper, visual_mapper::VisualMapper},
    repo::VisualRepo,
};

pub(crate) struct VisualLikeRepo;

impl VisualLikeRepo {
    /// 点赞影像.
    pub(crate) async fn like(
        state: &VisualState,
        user_id: UserId,
        visual_id: VisualId,
    ) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            // 确认存在
            VisualMapper::ensure_exist(txn, visual_id).await?;
            // 插入
            if !VisualLikeMapper::insert(txn, user_id, visual_id).await? {
                return Err(ContextualError::error_without_source(
                    "visual_already_liked",
                    "影像已经点赞过",
                    AppError::bad_request("已经点赞过"),
                ));
            }
            // 更新计数
            VisualMapper::update_like_count_delta(txn, visual_id, 1).await?;

            AuditRecorder::append(
                txn,
                AuditEvent::new("like")
                    .with_actor(user_id.0)
                    .with_target("visual", visual_id.0),
            )
            .await?;
            Ok(())
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        VisualRepo::cache_visual_like_status(state, user_id, visual_id, true).await;
        VisualRepo::invalidate_visual_info(state, visual_id).await;

        Ok(())
    }

    /// 取消点赞
    pub(crate) async fn unlike(
        state: &VisualState,
        user_id: UserId,
        visual_id: VisualId,
    ) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            if !VisualLikeMapper::delete(txn, user_id, visual_id).await? {
                return Err(ContextualError::error_without_source(
                    "visual_not_liked",
                    "影像尚未点赞",
                    AppError::bad_request("还未点赞"),
                ));
            }
            VisualMapper::update_like_count_delta(txn, visual_id, -1).await?;
            AuditRecorder::append(
                txn,
                AuditEvent::new("unlike")
                    .with_actor(user_id.0)
                    .with_target("visual", visual_id.0),
            )
            .await?;
            Ok(())
        })
        .timed(metrics_name!("db_transaction"))
        .await?;
        VisualRepo::cache_visual_like_status(state, user_id, visual_id, false).await;
        VisualRepo::invalidate_visual_info(state, visual_id).await;
        Ok(())
    }

    /// 查询用户点赞过的影像 ID.
    pub(crate) async fn query_liked_visual_ids(
        state: &VisualState,
        user_id: UserId,
        req: &LikedVisualsQuery,
    ) -> Result<CursorPage<(VisualId, DateTime), TimeIdCursor<VisualId>>> {
        VisualLikeMapper::query_user_liked_visual_ids(&state.db, user_id, &req.cursor, req.size)
            .timed(metrics_name!("query_ids"))
            .await
    }
}
