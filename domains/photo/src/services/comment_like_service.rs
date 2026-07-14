use common::{
    Result,
    error::AppError,
    ext::{ToErr, log_warn},
    metrics_group, metrics_success, metrics_timer_name, timed,
    utils::{DbUtils, MetricsTimerExt},
};
use entities::{auth::user::UserId, photo::comment::CommentId};

use crate::{
    mappers::{comment_like_mapper::CommentLikeMapper, comment_mapper::CommentMapper},
    state::PhotoState,
};

pub(crate) struct CommentLikeService;

// 创建
impl CommentLikeService {
    pub async fn like(state: &PhotoState, user_id: UserId, comment_id: CommentId) -> Result<()> {
        metrics_group!("like_comment");

        // 检查评论是否存在
        if !CommentMapper::exists(&state.db, comment_id)
            .timed(metrics_timer_name!("like_comment", "check_exists"))
            .await?
        {
            return log_warn(
                "comment_not_found",
                "用户尝试点赞不存在的评论",
                "",
                AppError::not_found("评论不存在"),
            )
            .to_err();
        }

        timed!("like_comment", "db_transaction", {
            DbUtils::write(&state.db, |txn| {
                Box::pin(async move {
                    let inserted =
                        CommentLikeMapper::insert_ignore(txn, user_id, comment_id).await?;

                    if !inserted {
                        return log_warn(
                            "comment_like_already_exist",
                            "用户尝试点赞一个已经点赞过的评论",
                            "",
                            AppError::bad_request("已经点赞过"),
                        )
                        .to_err();
                    }

                    // 增加点赞总数
                    CommentMapper::update_like_count_delta(txn, comment_id, 1).await?;
                    Ok(())
                })
            })
            .await
        })?;

        metrics_success!("like_comment");
        Ok(())
    }
}

// 修改
impl CommentLikeService {}

// 查询
impl CommentLikeService {}

// 删除
impl CommentLikeService {
    pub async fn unlike(state: &PhotoState, user_id: UserId, comment_id: CommentId) -> Result<()> {
        metrics_group!("unlike_comment");

        timed!("unlike_comment", "db_transaction", {
            DbUtils::write(&state.db, |txn| {
                Box::pin(async move {
                    let deleted = CommentLikeMapper::delete(txn, user_id, comment_id).await?;

                    if !deleted {
                        return log_warn(
                            "comment_like_already_exist",
                            "用户尝试取消点赞一个未点赞过的评论",
                            "",
                            AppError::bad_request("还未点赞"),
                        )
                        .to_err();
                    }

                    // 减少点赞总数
                    CommentMapper::update_like_count_delta(txn, comment_id, -1).await?;
                    Ok(())
                })
            })
            .await
        })?;

        metrics_success!("unlike_comment");
        Ok(())
    }
}
