use std::collections::HashSet;

use audit::{AuditEvent, AuditRecorder};
use common::{
    db_transaction,
    error::{AppError, ContextualError, contextual::Result},
    ext::{ContextOptionExt, ContextualResultExt},
    models::CursorPage,
};
use types::{
    auth::user::UserId,
    photo::{
        CommentCursorPageParam,
        comment::{CommentId, CommentRecord},
        dto::comment::{CommentPublishParam, HOT_COMMENT_MAX_COUNT, HOT_COMMENT_MIN_LIKES},
        photo::PhotoId,
    },
};

use crate::mappers::{
    comment_like_mapper::CommentLikeMapper, comment_mapper::CommentMapper,
    photo_mapper::PhotoMapper,
};
use crate::repo::PhotoRepo;
use crate::state::PhotoState;

pub(crate) struct CommentRepo;

impl CommentRepo {
    /// 查询热门评论和游标分页评论, 同时加载当前用户点赞状态.
    pub(crate) async fn query_comments(
        state: &PhotoState,
        user_id: UserId,
        photo_id: PhotoId,
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
                photo_id,
                HOT_COMMENT_MIN_LIKES,
                HOT_COMMENT_MAX_COUNT,
            )
            .await?
        } else {
            Vec::new()
        };

        // 排除掉热门评论后 再获取评论列表
        let exclude_ids = hot_comments
            .iter()
            .map(|comment| comment.id)
            .collect::<Vec<_>>();
        let comments = CommentMapper::query_by_photo_id(
            &state.db,
            photo_id,
            &exclude_ids,
            req.cursor.as_ref(),
            req.size,
        )
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
        .await?;

        Ok((hot_comments, comments, liked))
    }

    /// 发布评论.
    pub(crate) async fn publish_comment(
        state: &PhotoState,
        user_id: UserId,
        photo_id: PhotoId,
        req: CommentPublishParam,
    ) -> Result<types::photo::comment::CommentRecord> {
        let comment = db_transaction!(scoped & state.db, |txn| {
            let comment =
                CommentMapper::insert(txn, photo_id, user_id, req.content.into_inner()).await?;
            PhotoMapper::update_comment_count_delta(txn, photo_id, 1).await?;
            AuditRecorder::append(
                txn,
                AuditEvent::new("comment_publish")
                    .with_actor(user_id.0)
                    .with_target("photo", photo_id.0)
                    .with_detail(serde_json::json!({ "commentId": comment.id.0 })),
            )
            .await?;
            Ok(comment)
        })
        .await?;
        PhotoRepo::invalidate_photo_info(state, photo_id).await;
        Ok(comment)
    }

    /// 删除评论.
    /// 同时修改 评论like 和 照片评论计数
    pub(crate) async fn delete_comment(
        state: &PhotoState,
        user_id: UserId,
        comment_id: CommentId,
    ) -> Result<()> {
        let photo_id = db_transaction!(scoped & state.db, |txn| {
            // 删除评论
            let comment = CommentMapper::delete(txn, user_id, comment_id)
                .await?
                .context_warn_none(
                    "comment_delete_failed",
                    "删除评论失败",
                    AppError::bad_request("删除评论失败"),
                )?;

            // 更新照片评论计数
            // 错误仅记录
            PhotoMapper::update_comment_count_delta(txn, comment.photo_id, -1)
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
                    .with_target("photo", comment.photo_id)
                    .with_detail(serde_json::json!({ "commentId": comment_id.0 })),
            )
            .await?;
            Ok(comment.photo_id)
        })
        .await?;
        PhotoRepo::invalidate_photo_info(state, photo_id).await;
        Ok(())
    }

    pub async fn ensure_exist(state: &PhotoState, comment_id: CommentId) -> Result<()> {
        CommentMapper::ensure_exist(&state.db, comment_id).await
    }

    /// like评论.
    /// 同时修改点赞计数
    pub(crate) async fn like_comment(
        state: &PhotoState,
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
        .await
    }

    /// 取消点赞.
    /// 同时修改点赞计数
    pub(crate) async fn unlike_comment(
        state: &PhotoState,
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
        .await
    }
}
