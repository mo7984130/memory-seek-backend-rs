use crate::{
    VisualRepo,
    mappers::{comment_like_mapper::CommentLikeMapper, comment_mapper::CommentMapper},
    repo::CommentRepo,
    state::VisualState,
};
use common::{Result, ext::ToOk, types::CursorPage};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    visual::{
        comment::CommentId,
        dto::comment::{CommentCursorPageParam, CommentPublishParam, CommentView},
        visual::VisualId,
    },
};

pub(crate) struct CommentService;

// 创建
impl CommentService {
    /// 发布影像评论.
    #[common_macros::metered(name = "publish_comment")]
    #[tracing::instrument(
        name = "publish_comment",
        skip_all,
        fields(user_id = %user_id, visual_id = %visual_id)
    )]
    pub async fn publish(
        state: &VisualState,
        user_id: UserId,
        visual_id: VisualId,
        req: CommentPublishParam,
    ) -> Result<CommentView> {
        // 确认影像存在
        VisualRepo::ensure_exist(state, visual_id).await?;

        let comment = CommentRepo::publish_comment(state, user_id, visual_id, req).await?;

        CommentView::from(comment).to_ok()
    }
}

// 修改
impl CommentService {}

// 查询
impl CommentService {
    /// 获取影像评论列表.
    #[common_macros::metered(name = "get_comment_cursor_page")]
    #[tracing::instrument(
        name = "get_comment_cursor_page",
        skip_all,
        fields(user_id = %user_id, visual_id = %visual_id)
    )]
    pub async fn get_cursor_page(
        state: &VisualState,
        user_id: UserId,
        visual_id: VisualId,
        req: CommentCursorPageParam,
    ) -> Result<CursorPage<CommentView, TimeIdCursor<CommentId>>> {
        // 获取热门评论, 评论列表, 是否喜欢评论
        let (hot_comments, page, is_like) =
            CommentRepo::query_comments(state, user_id, visual_id, &req).await?;
        let page = page.with_next_cursor(|comment| TimeIdCursor {
            time_at: comment.created_at,
            id: comment.id,
        });

        // 组装结果
        page.map_records(|mut comments| {
            comments.extend(hot_comments);
            comments
                .into_iter()
                .map(|c| {
                    let is_liked = is_like.contains(&c.id);
                    CommentView::from(c).with_liked(is_liked)
                })
                .collect()
        })
        .to_ok()
    }
}

// 删除
impl CommentService {
    /// 删除评论.
    /// 同时会删除评论点赞
    #[common_macros::metered(name = "delete_comment")]
    #[tracing::instrument(
        name = "delete_comment",
        skip_all,
        fields(user_id = %user_id, comment_id = %comment_id)
    )]
    pub async fn delete(state: &VisualState, user_id: UserId, comment_id: CommentId) -> Result<()> {
        CommentRepo::delete_comment(state, user_id, comment_id).await?;

        Ok(())
    }
}

// 当影像删除时
// 删除评论 和 评论点赞
#[step_derive::declare_transaction_step(
    ctx = crate::services::visual_service::VisualDeleteContext,
    slice = crate::services::visual_service::MEDIA_DELETE_STEPS,
    name = "comment_cleanup",
    owns = ["CommentMapper", "CommentLikeMapper"],
    method = on_visual_delete,
)]
impl CommentService {
    async fn on_visual_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::visual_service::VisualDeleteContext,
    ) -> common::error::contextual::Result<()> {
        let visual_ids = ctx.visual_ids();
        let comment_ids = CommentMapper::delete_by_visual_ids(txn, &visual_ids).await?;

        if comment_ids.is_empty() {
            return Ok(());
        } else {
            CommentLikeMapper::delete_by_comment_ids(txn, &comment_ids).await?;
        }

        Ok(())
    }
}
