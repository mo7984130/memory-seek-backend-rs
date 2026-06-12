use crate::{
    mappers::{
        comment_like_mapper::CommentLikeMapper, comment_mapper::CommentMapper,
        photo_mapper::PhotoMapper,
    },
    models::comment::{
        COMMENT_CURSOR_PAGE_MAX_SIZE, HOT_COMMENT_MAX_COUNT, HOT_COMMENT_MIN_LIKES,
        PhotoCommentResult,
    },
    state::PhotoState,
};
use common::{
    Result,
    error::AppError,
    ext::{ToErr, ToOk, log_warn},
    models::CursorPage,
    utils::DbUtils,
};
use entities::{
    auth::user::UserId,
    photo::{comment::CommentId, photo::PhotoId},
};
use sea_orm::entity::prelude::DateTimeUtc;

pub(crate) struct CommentService;

// 创建
impl CommentService {
    pub async fn publish(
        state: &PhotoState,
        photo_id: PhotoId,
        user_id: UserId,
        content: String,
    ) -> Result<PhotoCommentResult> {
        let comment = DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                // 查询照片是否存在
                if PhotoMapper::exists(txn, photo_id).await? {
                    return log_warn(
                        "comment_publish_photo_not_exists",
                        "用户尝试评论不存在的照片",
                        "",
                        AppError::bad_request("无法评论不存在的照片"),
                    )
                    .to_err();
                }
                // 插入评论
                let comment = CommentMapper::insert(txn, photo_id, user_id, content).await?;
                // 更新评论总数
                PhotoMapper::update_comment_count_delta(txn, photo_id, 1).await?;
                Ok(comment)
            })
        })
        .await?;
        PhotoCommentResult::from(comment).to_ok()
    }
}

// 修改
impl CommentService {}

// 查询
impl CommentService {
    pub async fn get_cursor_page(
        state: &PhotoState,
        photo_id: PhotoId,
        user_id: UserId,
        cursor: Option<DateTimeUtc>,
        size: Option<u64>,
    ) -> Result<CursorPage<PhotoCommentResult, DateTimeUtc>> {
        // 校验 limit 参数
        let size = size.unwrap_or(32);
        if size > COMMENT_CURSOR_PAGE_MAX_SIZE {
            return log_warn(
                "comment_cursor_page_max_size",
                "用户获取评论, 传入的size超过最大值",
                "",
                AppError::bad_request("size超过最大值"),
            )
            .to_err();
        }

        // 如果是第一次(不带Cursor)获取的话, 展示热门评论
        let hot_comments = if cursor.is_none() {
            CommentMapper::query_hot_comments(
                &state.db,
                photo_id,
                HOT_COMMENT_MIN_LIKES,
                HOT_COMMENT_MAX_COUNT,
            )
            .await?
        } else {
            vec![]
        };

        // 获取评论
        let time_comments = CommentMapper::query_by_photo_id(
            &state.db,
            photo_id,
            hot_comments.iter().map(|comment| comment.id).collect(),
            cursor,
            size + 1,
        )
        .await?;

        let CursorPage {
            records: time_comments,
            has_more,
            ..
        } = CursorPage::from_oversize(time_comments, size);
        let mut comments = hot_comments;
        comments.extend(time_comments);

        let next_cursor = if has_more {
            comments.last().map(|comment| comment.created_at)
        } else {
            None
        };

        // 获取评论是否点赞
        let is_like = CommentLikeMapper::query_is_like_by_comment_ids(
            &state.db,
            user_id,
            comments.iter().map(|c| c.id).collect(),
        )
        .await?;

        let records = comments
            .into_iter()
            .map(|c| {
                let is_liked = is_like.contains(&c.id);
                PhotoCommentResult::from(c).with_liked(is_liked)
            })
            .collect();

        CursorPage {
            records,
            has_more,
            next_cursor,
        }
        .to_ok()
    }
}

// 删除
impl CommentService {
    pub async fn delete(state: &PhotoState, user_id: UserId, comment_id: CommentId) -> Result<()> {
        DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                let photo_id = CommentMapper::query_photo_id_by_id(txn, comment_id).await?;

                // 先删除评论, 在删除评论的同时, 校验权限
                let deleted = CommentMapper::delete(txn, user_id, comment_id).await?;
                if !deleted {
                    return log_warn(
                        "del_comment_not_deleted",
                        "用户尝试删除评论, 失败",
                        "",
                        AppError::bad_request("删除评论失败"),
                    )
                    .to_err();
                }

                // 更新照片评论数
                PhotoMapper::update_comment_count_delta(txn, photo_id, -1).await?;

                // 删除评论喜欢
                // 错误不返回
                let _ = CommentLikeMapper::delete_all_by_comment_id(txn, comment_id).await;

                Ok(())
            })
        })
        .await?;

        Ok(())
    }
}
