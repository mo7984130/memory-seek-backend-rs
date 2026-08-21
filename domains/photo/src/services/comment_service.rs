use crate::{
    PhotoRepo,
    mappers::{comment_like_mapper::CommentLikeMapper, comment_mapper::CommentMapper},
    repo::CommentRepo,
    state::PhotoState,
};
use common::{Result, ext::ToOk, models::CursorPage};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    photo::{
        comment::CommentId,
        dto::comment::{CommentCursorPageParam, CommentPublishParam, CommentView},
        photo::PhotoId,
    },
};

pub(crate) struct CommentService;

// 创建
impl CommentService {
    /// 发布照片评论.
    #[common::metered(name = "publish_comment")]
    #[tracing::instrument(
        name = "publish_comment",
        skip_all,
        fields(user_id = %user_id, photo_id = %photo_id)
    )]
    pub async fn publish(
        state: &PhotoState,
        user_id: UserId,
        photo_id: PhotoId,
        req: CommentPublishParam,
    ) -> Result<CommentView> {
        // 确认照片存在
        PhotoRepo::ensure_exist(state, photo_id).await?;

        let comment = CommentRepo::publish_comment(state, user_id, photo_id, req).await?;

        CommentView::from(comment).to_ok()
    }
}

// 修改
impl CommentService {}

// 查询
impl CommentService {
    /// 获取照片评论列表.
    #[common::metered(name = "get_comment_cursor_page")]
    #[tracing::instrument(
        name = "get_comment_cursor_page",
        skip_all,
        fields(user_id = %user_id, photo_id = %photo_id)
    )]
    pub async fn get_cursor_page(
        state: &PhotoState,
        user_id: UserId,
        photo_id: PhotoId,
        req: CommentCursorPageParam,
    ) -> Result<CursorPage<CommentView, TimeIdCursor<CommentId>>> {
        // 获取热门评论, 评论列表, 是否喜欢评论
        let (hot_comments, page, is_like) =
            CommentRepo::query_comments(state, user_id, photo_id, &req).await?;
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
    #[common::metered(name = "delete_comment")]
    #[tracing::instrument(
        name = "delete_comment",
        skip_all,
        fields(user_id = %user_id, comment_id = %comment_id)
    )]
    pub async fn delete(state: &PhotoState, user_id: UserId, comment_id: CommentId) -> Result<()> {
        CommentRepo::delete_comment(state, user_id, comment_id).await?;

        Ok(())
    }
}

// 当照片删除时
// 删除评论 和 评论点赞
#[step_derive::declare_transaction_step(
    ctx = crate::services::photo_service::PhotoDeleteContext,
    slice = crate::services::photo_service::PHOTO_DELETE_STEPS,
    name = "comment_cleanup",
    owns = ["CommentMapper", "CommentLikeMapper"],
)]
impl CommentService {
    async fn on_photo_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::photo_service::PhotoDeleteContext,
    ) -> common::Result<()> {
        let photo_ids = ctx.photo_ids();
        let comment_ids = CommentMapper::delete_by_photo_ids(txn, &photo_ids).await?;

        if comment_ids.is_empty() {
            return Ok(());
        } else {
            CommentLikeMapper::delete_by_comment_ids(txn, &comment_ids).await?;
        }

        Ok(())
    }
}
