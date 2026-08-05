use crate::{
    mappers::{
        comment_like_mapper::CommentLikeMapper, comment_mapper::CommentMapper,
        photo_mapper::PhotoMapper,
    },
    state::PhotoState,
};
use common::{
    Result,
    error::AppError,
    ext::{BoolExt, ToOk},
    metrics_group, metrics_name, metrics_success,
    models::CursorPage,
    timed,
    utils::{DbUtils, MetricsTimerExt},
};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    photo::{
        comment::CommentId,
        dto::comment::{
            CommentCursorPageParam, CommentPublishParam, CommentView, HOT_COMMENT_MAX_COUNT,
            HOT_COMMENT_MIN_LIKES,
        },
        photo::PhotoId,
    },
};

pub(crate) struct CommentService;

// 创建
impl CommentService {
    #[tracing::instrument(name = "publish_comment", skip_all)]
    pub async fn publish(
        state: &PhotoState,
        photo_id: PhotoId,
        user_id: UserId,
        param: CommentPublishParam,
    ) -> Result<CommentView> {
        metrics_group!();

        let CommentPublishParam { content } = param;

        let comment = timed!("db_transaction", {
            DbUtils::write(&state.db, |txn| {
                Box::pin(async move {
                    // 查询照片是否存在
                    PhotoMapper::ensure_exist(txn, photo_id).await?;

                    // 插入评论
                    let comment =
                        CommentMapper::insert(txn, photo_id, user_id, content.into_inner()).await?;
                    // 更新评论总数
                    PhotoMapper::update_comment_count_delta(txn, photo_id, 1).await?;
                    Ok(comment)
                })
            })
            .await
        })?;

        metrics_success!();
        CommentView::from(comment).to_ok()
    }
}

// 修改
impl CommentService {}

// 查询
impl CommentService {
    #[tracing::instrument(name = "get_comment_cursor_page", skip_all)]
    pub async fn get_cursor_page(
        state: &PhotoState,
        photo_id: PhotoId,
        user_id: UserId,
        param: CommentCursorPageParam,
    ) -> Result<CursorPage<CommentView, String>> {
        metrics_group!();

        // 如果是第一次(不带Cursor)获取的话, 展示热门评论
        let hot_comments = if param.cursor.is_none() {
            CommentMapper::query_hot_comments(
                &state.db,
                photo_id,
                HOT_COMMENT_MIN_LIKES,
                HOT_COMMENT_MAX_COUNT,
            )
            .timed(metrics_name!("query_hot_comments"))
            .await?
        } else {
            vec![]
        };

        // 获取评论
        let exclude_ids: Vec<CommentId> = hot_comments.iter().map(|comment| comment.id).collect();

        let time_comments = CommentMapper::query_by_photo_id(
            &state.db,
            photo_id,
            &exclude_ids,
            param.cursor.as_ref(),
            param.size,
        )
        .timed(metrics_name!("query_by_photo_id"))
        .await?;

        let CursorPage {
            records: time_comments,
            has_more,
            ..
        } = CursorPage::from_oversize(time_comments, param.size);
        let mut comments = hot_comments;
        comments.extend(time_comments);

        let next_cursor = if has_more {
            comments.last().map(|comment| {
                TimeIdCursor {
                    created_at: comment.created_at,
                    id: comment.id,
                }
                .encode()
            })
        } else {
            None
        };

        // 获取评论是否点赞
        let is_like = CommentLikeMapper::query_is_like_by_comment_ids(
            &state.db,
            user_id,
            comments.iter().map(|c| c.id).collect(),
        )
        .timed(metrics_name!("query_is_like"))
        .await?;

        let records = comments
            .into_iter()
            .map(|c| {
                let is_liked = is_like.contains(&c.id);
                CommentView::from(c).with_liked(is_liked)
            })
            .collect();

        metrics_success!();
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
    #[tracing::instrument(name = "delete_comment", skip_all)]
    pub async fn delete(state: &PhotoState, user_id: UserId, comment_id: CommentId) -> Result<()> {
        metrics_group!();

        timed!("db_transaction", {
            DbUtils::write(&state.db, |txn| {
                Box::pin(async move {
                    let photo_id = CommentMapper::query_photo_id_by_id(txn, comment_id).await?;

                    // 先删除评论, 在删除评论的同时, 校验权限
                    CommentMapper::delete(txn, user_id, comment_id)
                        .await?
                        .true_or_warn(
                            "del_comment_not_deleted",
                            "用户尝试删除评论, 失败",
                            AppError::bad_request("删除评论失败"),
                        )?;

                    // 更新照片评论数
                    PhotoMapper::update_comment_count_delta(txn, photo_id, -1).await?;

                    // 删除评论喜欢
                    // 错误不返回
                    let _ = CommentLikeMapper::delete_all_by_comment_id(txn, comment_id).await;

                    Ok(())
                })
            })
            .await
        })?;

        metrics_success!();
        Ok(())
    }
}
