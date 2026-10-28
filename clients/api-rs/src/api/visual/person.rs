//! 人物相关 API。
//!
//! 路由前缀:`/visual/person`(需鉴权)。
//!
//! 管理端接口(`admin/*`)需要管理员账号,文档中单独标注。

use crate::client::{Client, Request};
use crate::error::Result;
use common_core::types::CursorPage;
use types_core::cursor::{CountIdCursor, TimeIdCursor};
use types_core::{PersonId, VisualId};
use types_visual::{
    MergePersonParam, PersonCursorParam, PersonName, PersonSearchParam, PersonView,
    PersonVisualCursorParam, RenamePersonParam, SecondaryClusterParam, VisualView,
};

/// 获取人物列表(按人脸数倒序的游标分页)。`GET /visual/person`
pub async fn get_persons(
    client: &Client,
    cursor: Option<CountIdCursor<PersonId>>,
    size: Option<u64>,
) -> Result<CursorPage<PersonView, CountIdCursor<PersonId>>> {
    let param = PersonCursorParam {
        cursor,
        size: size.unwrap_or(32),
    };
    let mut request = Request::get("/visual/person");
    if let Some(cursor) = param.cursor {
        request = request.query("cursor", cursor);
    }
    request = request.query("size", param.size);
    client.send_json(request).await
}

/// 按完整名字或姓名首字母前缀搜索人物。`GET /visual/person/search`
pub async fn search_persons(
    client: &Client,
    keyword: &str,
    cursor: Option<PersonId>,
    size: Option<u64>,
) -> Result<CursorPage<PersonView, PersonId>> {
    let param = PersonSearchParam {
        keyword: keyword.to_owned(),
        cursor,
        size: size.unwrap_or(32),
    };
    let mut request = Request::get("/visual/person/search")
        .query("keyword", param.keyword)
        .query("size", param.size);
    if let Some(cursor) = param.cursor {
        request = request.query("cursor", cursor);
    }
    client.send_json(request).await
}

/// 获取人物关联的影像(游标分页)。`GET /visual/person/{personId}/visuals`
pub async fn get_person_visuals(
    client: &Client,
    person_id: PersonId,
    cursor: Option<TimeIdCursor<VisualId>>,
    size: Option<u64>,
) -> Result<CursorPage<VisualView, TimeIdCursor<VisualId>>> {
    let param = PersonVisualCursorParam {
        cursor,
        size: size.unwrap_or(32),
    };
    let mut request = Request::get(format!("/visual/person/{person_id}/visuals"));
    if let Some(cursor) = param.cursor {
        request = request.query("cursor", cursor);
    }
    request = request.query("size", param.size);
    client.send_json(request).await
}

/// 重命名人物。`POST /visual/person/{personId}/name`
pub async fn rename_person(
    client: &Client,
    person_id: PersonId,
    new_name: PersonName,
) -> Result<()> {
    let param = RenamePersonParam { new_name };
    client
        .send_unit(Request::post(format!("/visual/person/{person_id}/name")).json(&param))
        .await
}

/// 删除人物及其关联数据(管理员)。`DELETE /visual/person/{personId}`
pub async fn delete_person(client: &Client, person_id: PersonId) -> Result<()> {
    client
        .send_unit(Request::delete(format!("/visual/person/{person_id}")))
        .await
}

/// 合并两个人物,将源人物的人脸转移到目标人物(管理员)。
/// `POST /visual/person/merge`
pub async fn merge_person(
    client: &Client,
    source_person_id: PersonId,
    target_person_id: PersonId,
) -> Result<PersonView> {
    let param = MergePersonParam {
        source_person_id,
        target_person_id,
    };
    client
        .send_json(Request::post("/visual/person/merge").json(&param))
        .await
}

/// 启动全量人物扫描任务(管理员)。`GET /visual/person/admin/full_scan`
pub async fn full_scan(client: &Client) -> Result<()> {
    client
        .send_unit(Request::get("/visual/person/admin/full_scan"))
        .await
}

/// 对未分配人脸执行二次聚类指派(管理员)。
/// `POST /visual/person/admin/secondary_cluster`
pub async fn secondary_cluster(client: &Client, param: &SecondaryClusterParam) -> Result<()> {
    client
        .send_unit(Request::post("/visual/person/admin/secondary_cluster").json(param))
        .await
}
