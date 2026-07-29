use common::{
    Result, error::AppError, ext::BoolExt, metrics_group, metrics_success, timed, utils::DbUtils,
};
use types::{auth::user::UserId, photo::comment::CommentId};

use crate::{
    mappers::{comment_like_mapper::CommentLikeMapper, comment_mapper::CommentMapper},
    state::PhotoState,
};

pub(crate) struct CommentLikeService;

// 创建
impl CommentLikeService {
    pub async fn like(state: &PhotoState, user_id: UserId, comment_id: CommentId) -> Result<()> {
        metrics_group!();

        // 检查评论是否存在
        timed!("db_transaction", {
            DbUtils::write(&state.db, |txn| {
                Box::pin(async move {
                    CommentMapper::ensure_exist(txn, comment_id).await?;

                    CommentLikeMapper::insert(txn, user_id, comment_id)
                        .await?
                        .true_or_warn(
                            "comment_like_already_exist",
                            "用户尝试点赞一个已经点赞过的评论",
                            AppError::bad_request("已经点赞过"),
                        )?;

                    // 增加点赞总数
                    CommentMapper::update_like_count_delta(txn, comment_id, 1).await?;
                    Ok(())
                })
            })
            .await
        })?;

        metrics_success!();
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
        metrics_group!();

        timed!("db_transaction", {
            DbUtils::write(&state.db, |txn| {
                Box::pin(async move {
                    CommentLikeMapper::delete(txn, user_id, comment_id)
                        .await?
                        .true_or_warn(
                            "comment_like_already_exist",
                            "用户尝试取消点赞还未点赞过的",
                            AppError::bad_request("还未点赞"),
                        )?;

                    // 减少点赞总数
                    CommentMapper::update_like_count_delta(txn, comment_id, -1).await?;
                    Ok(())
                })
            })
            .await
        })?;

        metrics_success!();
        Ok(())
    }
}
