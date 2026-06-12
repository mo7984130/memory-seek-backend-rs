use common::{
    Result,
    error::AppError,
    ext::{ToErr, log_warn},
    utils::DbUtils,
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
        // 检查评论是否存在
        if !CommentMapper::exists(&state.db, comment_id).await? {
            return log_warn(
                "comment_not_found",
                "用户尝试点赞不存在的评论",
                "",
                AppError::not_found("评论不存在"),
            )
            .to_err();
        }

        DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                let inserted = CommentLikeMapper::insert_ignore(txn, user_id, comment_id).await?;

                if !inserted {
                    return log_warn(
                        "comment_like_already_exist",
                        "用户尝试点赞一个已经点赞过的评论",
                        "",
                        AppError::bad_request("已经点赞过"),
                    )
                    .to_err();
                }

                // // redis 增加点赞数, 错误不返回
                // let _: Result<i64> = state
                //     .redis
                //     .get_conn()
                //     .await?
                //     .incr(RedisKeys::likes_count(comment_id), 1)
                //     .await
                //     .trace_internal_err("redis_incr_err", "增加照片评论点赞数redis错误");

                // 增加点赞总数
                CommentMapper::update_like_count_delta(txn, comment_id, 1).await?;
                Ok(())
            })
        })
        .await?;
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

                // // redis 减少点赞数, 错误不返回
                // let script = redis::Script::new(
                //     r#"
                //     local current = redis.call('GET', KEYS[1])
                //     if current and tonumber(current) > 0 then
                //         return redis.call('DECR', KEYS[1])
                //     else
                //         return 0
                //     end
                // "#,
                // );

                // let mut conn = state.redis.get_conn().await?;
                // script
                //     .key(RedisKeys::likes_count(comment_id))
                //     .invoke_async::<i64>(&mut conn)
                //     .await
                //     .trace_internal_err("redis_decr_err", "减少照片评论点赞数redis错误")?;

                // 减少点赞总数
                CommentMapper::update_like_count_delta(txn, comment_id, -1).await?;
                Ok(())
            })
        })
        .await?;
        Ok(())
    }
}
