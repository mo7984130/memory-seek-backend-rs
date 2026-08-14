use crate::{
    mappers::{comment_like_mapper::CommentLikeMapper, comment_mapper::CommentMapper},
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
        let comment = state.repo.publish_comment(user_id, photo_id, req).await?;

        CommentView::from(comment).to_ok()
    }
}

// 修改
impl CommentService {}

// 查询
impl CommentService {
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
    ) -> Result<CursorPage<CommentView, String>> {
        let size = req.size;
        let (hot_comments, time_comments, is_like) =
            state.repo.query_comments(user_id, photo_id, &req).await?;

        let CursorPage {
            records: time_comments,
            has_more,
            ..
        } = CursorPage::from_oversize(time_comments, size);
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

        let records = comments
            .into_iter()
            .map(|c| {
                let is_liked = is_like.contains(&c.id);
                CommentView::from(c).with_liked(is_liked)
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
    #[common::metered(name = "delete_comment")]
    #[tracing::instrument(
        name = "delete_comment",
        skip_all,
        fields(user_id = %user_id, comment_id = %comment_id)
    )]
    pub async fn delete(state: &PhotoState, user_id: UserId, comment_id: CommentId) -> Result<()> {
        state.repo.delete_comment(user_id, comment_id).await?;

        Ok(())
    }
}

// 照片删除步骤:评论清理
#[step_derive::declare_step(
    ctx = crate::repo::photo_repo::PhotoDeleteContext,
    slice = crate::repo::photo_repo::PHOTO_DELETE_STEPS,
    name = "comment_cleanup",
    owns = ["CommentMapper", "CommentLikeMapper"],
)]
impl CommentService {
    async fn on_photo_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::repo::photo_repo::PhotoDeleteContext,
    ) -> common::Result<()> {
        let photo_ids = ctx.photo_ids();
        let comment_ids = CommentMapper::delete_by_photo_ids(txn, &photo_ids).await?;
        if !comment_ids.is_empty() {
            CommentLikeMapper::delete_by_comment_ids(txn, &comment_ids).await?;
        }
        Ok(())
    }
}
