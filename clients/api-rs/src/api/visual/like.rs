//! 影像点赞相关 API。
//!
//! 路由前缀:`/visual/visual`。

use crate::client::{Client, Request};
use crate::error::Result;
use common_core::types::CursorPage;
use types_core::VisualId;
use types_core::cursor::TimeIdCursor;
use types_visual::dto::visual::VisualView;

/// 点赞影像。`POST /visual/visual/{visualId}/like`
pub async fn like_visual(client: &Client, visual_id: VisualId) -> Result<()> {
    client
        .send_unit(Request::post(format!("/visual/visual/{visual_id}/like")))
        .await
}

/// 取消点赞影像。`DELETE /visual/visual/{visualId}/like`
pub async fn unlike_visual(client: &Client, visual_id: VisualId) -> Result<()> {
    client
        .send_unit(Request::delete(format!("/visual/visual/{visual_id}/like")))
        .await
}

/// 获取用户点赞的影像列表(游标分页)。`GET /visual/visual/liked`
pub async fn get_liked_visuals(
    client: &Client,
    cursor: Option<TimeIdCursor<VisualId>>,
    size: Option<u64>,
) -> Result<CursorPage<VisualView, TimeIdCursor<VisualId>>> {
    let mut request = Request::get("/visual/visual/liked");
    if let Some(cursor) = cursor {
        request = request.query("cursor", cursor);
    }
    if let Some(size) = size {
        request = request.query("size", size);
    }
    client.send_json(request).await
}
