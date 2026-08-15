use audit::{AuditEvent, AuditService};
use common::{
    error::{AppError, contextual::Result},
    models::CursorPage,
};
use types::{
    auth::user::UserId,
    photo::{
        comment::CommentId,
        dto::comment::{CommentPublishParam, HOT_COMMENT_MAX_COUNT, HOT_COMMENT_MIN_LIKES},
        photo::PhotoId,
    },
};

use super::PhotoRepo;
use crate::mappers::{
    comment_like_mapper::CommentLikeMapper, comment_mapper::CommentMapper,
    photo_mapper::PhotoMapper,
};

impl PhotoRepo {
    /// 查询热门评论和游标分页评论, 同时加载当前用户点赞状态.
    pub(crate) async fn query_comments(
        &self,
        user_id: UserId,
        photo_id: PhotoId,
        req: &types::photo::dto::comment::CommentCursorPageParam,
    ) -> common::error::contextual::Result<(
        Vec<types::photo::comment::CommentRecord>,
        CursorPage<types::photo::comment::CommentRecord, ()>,
        std::collections::HashSet<CommentId>,
    )> {
        let hot_comments = if req.cursor.is_none() {
            CommentMapper::query_hot_comments(
                &self.db,
                photo_id,
                HOT_COMMENT_MIN_LIKES,
                HOT_COMMENT_MAX_COUNT,
            )
            .await?
        } else {
            Vec::new()
        };
        let exclude_ids = hot_comments
            .iter()
            .map(|comment| comment.id)
            .collect::<Vec<_>>();
        let comments = CursorPage::from_oversize(
            CommentMapper::query_by_photo_id(
                &self.db,
                photo_id,
                &exclude_ids,
                req.cursor.as_ref(),
                req.size,
            )
            .await?,
            req.size,
        );
        let liked = CommentLikeMapper::query_is_like_by_comment_ids(
            &self.db,
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

    /// 校验照片存在后发布评论.
    pub(crate) async fn publish_comment(
        &self,
        user_id: UserId,
        photo_id: PhotoId,
        req: CommentPublishParam,
    ) -> Result<types::photo::comment::CommentRecord> {
        self.transaction(|txn| {
            Box::pin(async move {
                PhotoMapper::ensure_exist(txn, photo_id).await?;
                let comment =
                    CommentMapper::insert(txn, photo_id, user_id, req.content.into_inner()).await?;
                PhotoMapper::update_comment_count_delta(txn, photo_id, 1).await?;
                AuditService::append(
                    txn,
                    AuditEvent::new("photo.comment_published")
                        .with_actor(user_id.0)
                        .with_target("comment", comment.id.0),
                )
                .await?;
                Ok(comment)
            })
        })
        .await
    }

    /// 删除评论并同步维护照片评论计数.
    pub(crate) async fn delete_comment(
        &self,
        user_id: UserId,
        comment_id: CommentId,
    ) -> Result<()> {
        self.transaction(|txn| {
            Box::pin(async move {
                let photo_id = CommentMapper::query_photo_id_by_id(txn, comment_id).await?;
                if !CommentMapper::delete(txn, user_id, comment_id).await? {
                    return Err(AppError::bad_request("删除评论失败"));
                }
                PhotoMapper::update_comment_count_delta(txn, photo_id, -1).await?;
                let _ = CommentLikeMapper::delete_all_by_comment_id(txn, comment_id).await;
                AuditService::append(
                    txn,
                    AuditEvent::new("photo.comment_deleted")
                        .with_actor(user_id.0)
                        .with_target("comment", comment_id.0),
                )
                .await?;
                Ok(())
            })
        })
        .await
    }

    /// 校验评论存在后创建评论点赞.
    pub(crate) async fn like_comment(&self, user_id: UserId, comment_id: CommentId) -> Result<()> {
        self.transaction(|txn| {
            Box::pin(async move {
                CommentMapper::ensure_exist(txn, comment_id).await?;
                if !CommentLikeMapper::insert(txn, user_id, comment_id).await? {
                    return Err(AppError::bad_request("已经点赞过"));
                }
                CommentMapper::update_like_count_delta(txn, comment_id, 1).await?;
                AuditService::append(
                    txn,
                    AuditEvent::new("photo.comment_liked")
                        .with_actor(user_id.0)
                        .with_target("comment", comment_id.0),
                )
                .await?;
                Ok(())
            })
        })
        .await
    }

    /// 删除用户对评论的点赞并更新计数.
    pub(crate) async fn unlike_comment(
        &self,
        user_id: UserId,
        comment_id: CommentId,
    ) -> Result<()> {
        self.transaction(|txn| {
            Box::pin(async move {
                if !CommentLikeMapper::delete(txn, user_id, comment_id).await? {
                    return Err(AppError::bad_request("还未点赞"));
                }
                CommentMapper::update_like_count_delta(txn, comment_id, -1).await?;
                AuditService::append(
                    txn,
                    AuditEvent::new("photo.comment_unliked")
                        .with_actor(user_id.0)
                        .with_target("comment", comment_id.0),
                )
                .await?;
                Ok(())
            })
        })
        .await
    }
}
