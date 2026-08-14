use common::Result;
use types::{auth::user::UserId, photo::comment::CommentId};

use crate::state::PhotoState;

pub(crate) struct CommentLikeService;

// 创建
impl CommentLikeService {
    /// 为评论点赞; 重复点赞和不存在评论由仓储层处理.
    #[common::metered(name = "like_comment")]
    #[tracing::instrument(
        name = "like_comment",
        skip_all,
        fields(user_id = %user_id, comment_id = %comment_id)
    )]
    pub async fn like(state: &PhotoState, user_id: UserId, comment_id: CommentId) -> Result<()> {
        // 检查评论是否存在
        state.repo.like_comment(user_id, comment_id).await?;

        Ok(())
    }
}

// 修改
impl CommentLikeService {}

// 查询
impl CommentLikeService {}

// 删除
impl CommentLikeService {
    /// 取消用户对评论的点赞.
    #[common::metered(name = "unlike_comment")]
    #[tracing::instrument(
        name = "unlike_comment",
        skip_all,
        fields(user_id = %user_id, comment_id = %comment_id)
    )]
    pub async fn unlike(state: &PhotoState, user_id: UserId, comment_id: CommentId) -> Result<()> {
        state.repo.unlike_comment(user_id, comment_id).await?;

        Ok(())
    }
}
