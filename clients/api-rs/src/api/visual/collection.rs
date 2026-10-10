//! 收藏夹相关 API。
//!
//! 路由前缀:`/visual/collections`。
//!
use crate::client::{Client, Request};
use crate::error::Result;
use common_core::types::CursorPage;
use types_core::cursor::TimeIdCursor;
use types_core::{CollectionId, VisualId};
use types_visual::{
    CollectionBriefView, CollectionCreateParam, CollectionUpdateParam, CollectionView,
    CollectionVisualAddBatchParam, CollectionVisualAddBatchResult,
    CollectionVisualRemoveBatchParam, CollectionVisualRemoveBatchResult, VisualView,
};

/// 获取收藏夹列表。`GET /visual/collections`
pub async fn get_collection_list(client: &Client) -> Result<Vec<CollectionView>> {
    client.send_json(Request::get("/visual/collections")).await
}

/// 创建收藏夹。`POST /visual/collections`
pub async fn create_collection(
    client: &Client,
    param: &CollectionCreateParam,
) -> Result<CollectionView> {
    client
        .send_json(Request::post("/visual/collections").json(param))
        .await
}

/// 更新收藏夹信息。`PATCH /visual/collections/{id}`
pub async fn update_collection(
    client: &Client,
    collection_id: CollectionId,
    param: &CollectionUpdateParam,
) -> Result<()> {
    client
        .send_unit(Request::patch(format!("/visual/collections/{collection_id}")).json(param))
        .await
}

/// 删除收藏夹。`DELETE /visual/collections/{id}`
pub async fn delete_collection(client: &Client, collection_id: CollectionId) -> Result<()> {
    client
        .send_unit(Request::delete(format!(
            "/visual/collections/{collection_id}"
        )))
        .await
}

/// 获取收藏夹内的影像(游标分页)。`GET /visual/collections/{id}/visuals`
pub async fn get_collection_visuals(
    client: &Client,
    collection_id: CollectionId,
    cursor: Option<TimeIdCursor<VisualId>>,
    size: Option<u64>,
) -> Result<CursorPage<VisualView, TimeIdCursor<VisualId>>> {
    let mut request = Request::get(format!("/visual/collections/{collection_id}/visuals"));
    if let Some(cursor) = cursor {
        request = request.query("cursor", cursor);
    }
    if let Some(size) = size {
        request = request.query("size", size);
    }
    client.send_json(request).await
}

/// 批量添加影像到收藏夹。`POST /visual/collections/{id}/visuals`
pub async fn add_visuals_to_collection(
    client: &Client,
    collection_id: CollectionId,
    param: &CollectionVisualAddBatchParam,
) -> Result<CollectionVisualAddBatchResult> {
    client
        .send_json(
            Request::post(format!("/visual/collections/{collection_id}/visuals")).json(param),
        )
        .await
}

/// 批量从收藏夹移除影像。`DELETE /visual/collections/{id}/visuals`
pub async fn remove_visuals_from_collection(
    client: &Client,
    collection_id: CollectionId,
    param: &CollectionVisualRemoveBatchParam,
) -> Result<CollectionVisualRemoveBatchResult> {
    client
        .send_json(
            Request::delete(format!("/visual/collections/{collection_id}/visuals")).json(param),
        )
        .await
}

/// 从收藏夹移除单张影像。`DELETE /visual/collections/{id}/visuals/{visualId}`
pub async fn remove_visual_from_collection(
    client: &Client,
    collection_id: CollectionId,
    visual_id: VisualId,
) -> Result<()> {
    client
        .send_unit(Request::delete(format!(
            "/visual/collections/{collection_id}/visuals/{visual_id}"
        )))
        .await
}

/// 获取影像所属的收藏夹列表。`GET /visual/collections/by-visual/{visualId}`
pub async fn get_collections_by_visual(
    client: &Client,
    visual_id: VisualId,
) -> Result<Vec<CollectionBriefView>> {
    client
        .send_json(Request::get(format!(
            "/visual/collections/by-visual/{visual_id}"
        )))
        .await
}
