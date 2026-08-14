use common::error::{AppError, contextual::Result};
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
    pub(crate) async fn query_comments(
        &self,
        user_id: UserId,
        photo_id: PhotoId,
        req: &types::photo::dto::comment::CommentCursorPageParam,
    ) -> common::error::contextual::Result<(
        Vec<types::photo::comment::CommentRecord>,
        Vec<types::photo::comment::CommentRecord>,
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
        let comments = CommentMapper::query_by_photo_id(
            &self.db,
            photo_id,
            &exclude_ids,
            req.cursor.as_ref(),
            req.size,
        )
        .await?;
        let liked = CommentLikeMapper::query_is_like_by_comment_ids(
            &self.db,
            user_id,
            hot_comments
                .iter()
                .chain(&comments)
                .map(|comment| comment.id)
                .collect(),
        )
        .await?;
        Ok((hot_comments, comments, liked))
    }

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
                Ok(comment)
            })
        })
        .await
    }

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
                Ok(())
            })
        })
        .await
    }

    pub(crate) async fn like_comment(&self, user_id: UserId, comment_id: CommentId) -> Result<()> {
        self.transaction(|txn| {
            Box::pin(async move {
                CommentMapper::ensure_exist(txn, comment_id).await?;
                if !CommentLikeMapper::insert(txn, user_id, comment_id).await? {
                    return Err(AppError::bad_request("已经点赞过"));
                }
                CommentMapper::update_like_count_delta(txn, comment_id, 1).await?;
                Ok(())
            })
        })
        .await
    }

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
                Ok(())
            })
        })
        .await
    }
}
