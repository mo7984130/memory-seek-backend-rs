use std::collections::HashSet;

use audit::{AuditEvent, AuditRecorder};
use common::{
    db_transaction,
    error::contextual::ext::{ContextualResultExt, OptionExt},
    error::{AppError, ContextualError, contextual::Result},
    metrics_name,
    types::CursorPage,
    utils::MetricsTimerExt,
};
use types::{
    auth::user::UserId,
    media::{
        CommentCursorPageParam,
        comment::{CommentId, CommentRecord},
        dto::comment::{CommentPublishParam, HOT_COMMENT_MAX_COUNT, HOT_COMMENT_MIN_LIKES},
        media::MediaId,
    },
};

use crate::mappers::{
    comment_like_mapper::CommentLikeMapper, comment_mapper::CommentMapper,
    media_mapper::MediaMapper,
};
use crate::repo::MediaRepo;
use crate::state::MediaState;

pub(crate) struct CommentRepo;

impl CommentRepo {
    /// 查询热门评论和游标分页评论, 同时加载当前用户点赞状态.
    pub(crate) async fn query_comments(
        state: &MediaState,
        user_id: UserId,
        media_id: MediaId,
        req: &CommentCursorPageParam,
    ) -> Result<(
        Vec<CommentRecord>,
        CursorPage<CommentRecord, ()>,
        HashSet<CommentId>,
    )> {
        // 获取热门评论
        let hot_comments = if req.cursor.is_none() {
            CommentMapper::query_hot_comments(
                &state.db,
                media_id,
                HOT_COMMENT_MIN_LIKES,
                HOT_COMMENT_MAX_COUNT,
            )
            .timed(metrics_name!("query_hot_comments"))
            .await?
        } else {
            Vec::new()
        };

        // 排除掉热门评论后 再获取评论列表
        let exclude_ids = hot_comments
            .iter()
            .map(|comment| comment.id)
            .collect::<Vec<_>>();
        let comments = CommentMapper::query_by_media_id(
            &state.db,
            media_id,
            &exclude_ids,
            req.cursor.as_ref(),
            req.size,
        )
        .timed(metrics_name!("query_by_media_id"))
        .await?;

        // 获取评论是否喜欢
        let liked = CommentLikeMapper::query_is_like_by_comment_ids(
            &state.db,
            user_id,
            hot_comments
                .iter()
                .chain(&comments.records)
                .map(|comment| comment.id)
                .collect(),
        )
        .timed(metrics_name!("query_is_like"))
        .await?;

        Ok((hot_comments, comments, liked))
    }

    /// 发布评论.
    pub(crate) async fn publish_comment(
        state: &MediaState,
        user_id: UserId,
        media_id: MediaId,
        req: CommentPublishParam,
    ) -> Result<types::media::comment::CommentRecord> {
        let comment = db_transaction!(scoped & state.db, |txn| {
            let comment =
                CommentMapper::insert(txn, media_id, user_id, req.content.into_inner()).await?;
            MediaMapper::update_comment_count_delta(txn, media_id, 1).await?;
            AuditRecorder::append(
                txn,
                AuditEvent::new("comment_publish")
                    .with_actor(user_id.0)
                    .with_target("media", media_id.0)
                    .with_detail(serde_json::json!({ "commentId": comment.id.0 })),
            )
            .await?;
            Ok(comment)
        })
        .timed(metrics_name!("db_transaction"))
        .await?;
        MediaRepo::invalidate_media_info(state, media_id).await;
        Ok(comment)
    }

    /// 删除评论.
    /// 同时修改 评论like 和 媒体评论计数
    pub(crate) async fn delete_comment(
        state: &MediaState,
        user_id: UserId,
        comment_id: CommentId,
    ) -> Result<()> {
        let media_id = db_transaction!(scoped & state.db, |txn| {
            // 删除评论
            let comment = CommentMapper::delete(txn, user_id, comment_id)
                .await?
                .ok_or_warn(
                    "comment_delete_failed",
                    "删除评论失败",
                    AppError::bad_request("删除评论失败"),
                )?;

            // 更新媒体评论计数
            // 错误仅记录
            MediaMapper::update_comment_count_delta(txn, comment.media_id, -1)
                .await
                .emit_if_err();

            // 删除评论like
            // 错误仅记录
            CommentLikeMapper::delete_all_by_comment_id(txn, comment_id)
                .await
                .emit_if_err();

            AuditRecorder::append(
                txn,
                AuditEvent::new("comment_delete")
                    .with_actor(user_id.0)
                    .with_target("media", comment.media_id)
                    .with_detail(serde_json::json!({ "commentId": comment_id.0 })),
            )
            .await?;
            Ok(comment.media_id)
        })
        .timed(metrics_name!("db_transaction"))
        .await?;
        MediaRepo::invalidate_media_info(state, media_id).await;
        Ok(())
    }

    pub async fn ensure_exist(state: &MediaState, comment_id: CommentId) -> Result<()> {
        CommentMapper::ensure_exist(&state.db, comment_id)
            .timed(metrics_name!("db_query"))
            .await
    }

    /// like评论.
    /// 同时修改点赞计数
    pub(crate) async fn like_comment(
        state: &MediaState,
        user_id: UserId,
        comment_id: CommentId,
    ) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            // 插入记录
            if !CommentLikeMapper::insert(txn, user_id, comment_id).await? {
                return Err(ContextualError::error_without_source(
                    "comment_already_liked",
                    "评论已经点赞过",
                    AppError::bad_request("已经点赞过"),
                ));
            }

            // 更新like计数
            CommentMapper::update_like_count_delta(txn, comment_id, 1).await?;

            AuditRecorder::append(
                txn,
                AuditEvent::new("comment_like")
                    .with_actor(user_id.0)
                    .with_target("comment", comment_id.0),
            )
            .await?;
            Ok(())
        })
        .timed(metrics_name!("db_transaction"))
        .await
    }

    /// 取消点赞.
    /// 同时修改点赞计数
    pub(crate) async fn unlike_comment(
        state: &MediaState,
        user_id: UserId,
        comment_id: CommentId,
    ) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            // 删除点赞记录
            if !CommentLikeMapper::delete(txn, user_id, comment_id).await? {
                return Err(ContextualError::error_without_source(
                    "comment_not_liked",
                    "评论尚未点赞",
                    AppError::bad_request("还未点赞"),
                ));
            }
            // 更新点赞计数
            CommentMapper::update_like_count_delta(txn, comment_id, -1).await?;

            AuditRecorder::append(
                txn,
                AuditEvent::new("comment_unlike")
                    .with_actor(user_id.0)
                    .with_target("comment", comment_id.0),
            )
            .await?;
            Ok(())
        })
        .timed(metrics_name!("db_transaction"))
        .await
    }
}
