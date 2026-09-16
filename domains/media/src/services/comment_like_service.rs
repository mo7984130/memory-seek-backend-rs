use common::Result;
use types::{auth::user::UserId, media::comment::CommentId};

use crate::{repo::CommentRepo, state::MediaState};

pub(crate) struct CommentLikeService;

// 创建
impl CommentLikeService {
    /// 点赞评论.
    #[common_macros::metered(name = "like_comment")]
    #[tracing::instrument(
        name = "like_comment",
        skip_all,
        fields(user_id = %user_id, comment_id = %comment_id)
    )]
    pub async fn like(state: &MediaState, user_id: UserId, comment_id: CommentId) -> Result<()> {
        // 检查评论是否存在
        CommentRepo::ensure_exist(state, comment_id).await?;

        // like评论
        CommentRepo::like_comment(state, user_id, comment_id).await?;

        Ok(())
    }
}

// 修改
impl CommentLikeService {}

// 查询
impl CommentLikeService {}

// 删除
impl CommentLikeService {
    /// 取消点赞.
    #[common_macros::metered(name = "unlike_comment")]
    #[tracing::instrument(
        name = "unlike_comment",
        skip_all,
        fields(user_id = %user_id, comment_id = %comment_id)
    )]
    pub async fn unlike(state: &MediaState, user_id: UserId, comment_id: CommentId) -> Result<()> {
        CommentRepo::unlike_comment(state, user_id, comment_id).await?;

        Ok(())
    }
}
