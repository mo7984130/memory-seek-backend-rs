//! 人脸相关 API。
//!
//! 路由前缀:`/visual/face`(需鉴权)。
//!
//! 管理端接口(`admin/*`)需要管理员账号,文档中单独标注。

use crate::client::{Client, Request};
use crate::error::Result;
use common_core::types::CursorPage;
use types_core::cursor::TimeIdCursor;
use types_core::{FaceId, PersonId, VisualId};
use types_visual::{
    FaceDeleteBatchParam, FaceDeleteBatchResult, FaceIds, FaceView,
    UnassignedFaceVisualCursorParam, VisualView,
};

/// 获取影像中的人脸列表。`GET /visual/face/visual/{visualId}`
pub async fn get_faces(client: &Client, visual_id: VisualId) -> Result<Vec<FaceView>> {
    client
        .send_json(Request::get(format!("/visual/face/visual/{visual_id}")))
        .await
}

/// 获取当前用户“包含未分配人脸”的影像列表(游标分页)。
/// `GET /visual/face/unassigned-visuals`
pub async fn get_unassigned_face_visuals(
    client: &Client,
    cursor: Option<TimeIdCursor<VisualId>>,
    size: Option<u64>,
) -> Result<CursorPage<VisualView, TimeIdCursor<VisualId>>> {
    let param = UnassignedFaceVisualCursorParam {
        cursor,
        size: size.unwrap_or(32),
    };
    let mut request = Request::get("/visual/face/unassigned-visuals");
    if let Some(cursor) = param.cursor {
        request = request.query("cursor", cursor);
    }
    request = request.query("size", param.size);
    client.send_json(request).await
}

/// 修改人脸归属。`POST /visual/face/feature/{faceId}/belonging[/{personId}]`
///
/// `person_id` 为 `None` 时调用取消归属接口。
pub async fn change_face_belonging(
    client: &Client,
    face_id: FaceId,
    person_id: Option<PersonId>,
) -> Result<()> {
    let path = match person_id {
        Some(person_id) => {
            format!("/visual/face/feature/{face_id}/belonging/{person_id}")
        }
        None => format!("/visual/face/feature/{face_id}/belonging"),
    };
    client.send_unit(Request::post(path)).await
}

/// 删除一张未归属人物的人脸。`DELETE /visual/face/feature/{faceId}`
pub async fn delete_face(client: &Client, face_id: FaceId) -> Result<()> {
    client
        .send_unit(Request::delete(format!("/visual/face/feature/{face_id}")))
        .await
}

/// 批量删除未归属人物的人脸,已归属人脸会被后端跳过。
/// `DELETE /visual/face/feature`
pub async fn delete_faces_batch(
    client: &Client,
    face_ids: FaceIds,
) -> Result<FaceDeleteBatchResult> {
    let param = FaceDeleteBatchParam { face_ids };
    client
        .send_json(Request::delete("/visual/face/feature").json(&param))
        .await
}

/// 启动全量人脸计算任务(管理员)。`GET /visual/face/admin/full`
pub async fn full_compute(client: &Client) -> Result<()> {
    client
        .send_unit(Request::get("/visual/face/admin/full"))
        .await
}

/// 启动增量人脸计算任务(管理员)。`GET /visual/face/admin/incremental`
pub async fn incremental_compute(client: &Client) -> Result<()> {
    client
        .send_unit(Request::get("/visual/face/admin/incremental"))
        .await
}
