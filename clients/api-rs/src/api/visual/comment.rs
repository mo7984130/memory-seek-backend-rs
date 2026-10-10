//! 评论相关 API。
//!
//! 路由前缀:`/visual/comment`。

use crate::client::{Client, Request};
use crate::error::Result;
use common_core::types::CursorPage;
use types_core::cursor::TimeIdCursor;
use types_core::{CommentId, VisualId};
use types_visual::{CommentPublishParam, CommentView};

/// 获取评论列表(游标分页)。`GET /visual/comment/{visualId}`
pub async fn get_comment_list(
    client: &Client,
    visual_id: VisualId,
    cursor: Option<TimeIdCursor<CommentId>>,
    size: Option<u64>,
) -> Result<CursorPage<CommentView, TimeIdCursor<CommentId>>> {
    let mut request = Request::get(format!("/visual/comment/{visual_id}"));
    if let Some(cursor) = cursor {
        request = request.query("cursor", cursor);
    }
    if let Some(size) = size {
        request = request.query("size", size);
    }
    client.send_json(request).await
}

/// 发布评论。`POST /visual/comment/{visualId}`
pub async fn publish_comment(
    client: &Client,
    visual_id: VisualId,
    param: &CommentPublishParam,
) -> Result<CommentView> {
    client
        .send_json(Request::post(format!("/visual/comment/{visual_id}")).json(param))
        .await
}

/// 删除评论。`DELETE /visual/comment/{visualId}/{commentId}`
pub async fn delete_comment(
    client: &Client,
    visual_id: VisualId,
    comment_id: CommentId,
) -> Result<()> {
    client
        .send_unit(Request::delete(format!(
            "/visual/comment/{visual_id}/{comment_id}"
        )))
        .await
}

/// 点赞评论。`POST /visual/comment/{commentId}/like`
pub async fn like_comment(client: &Client, comment_id: CommentId) -> Result<()> {
    client
        .send_unit(Request::post(format!("/visual/comment/{comment_id}/like")))
        .await
}

/// 取消点赞评论。`DELETE /visual/comment/{commentId}/like`
pub async fn unlike_comment(client: &Client, comment_id: CommentId) -> Result<()> {
    client
        .send_unit(Request::delete(format!(
            "/visual/comment/{comment_id}/like"
        )))
        .await
}
