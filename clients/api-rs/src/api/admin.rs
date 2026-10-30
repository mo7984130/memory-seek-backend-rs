//! 管理端查询与后台任务 API。

use crate::client::{Client, Request};
use crate::error::Result;
use common_core::types::CursorPage;
use types_admin::{
    dashboard::VisualDashboardStats,
    job::{AdminJobQuery, AdminJobView, StartAdminJobRequest},
    user::AdminUserQuery,
};
use types_core::{UserId, cursor::TimeIdCursor};
use types_identity::user::UserInfo;

/// 获取管理首页的全站影像统计。
pub async fn get_visual_dashboard(client: &Client) -> Result<VisualDashboardStats> {
    client
        .send_json(Request::get("/admin/dashboard/visuals"))
        .await
}

/// 分页查询用户，可按用户名或邮箱模糊搜索。
pub async fn get_users(
    client: &Client,
    keyword: Option<&str>,
    cursor: Option<UserId>,
    size: Option<u64>,
) -> Result<CursorPage<UserInfo, UserId>> {
    let query = AdminUserQuery {
        keyword: keyword.map(str::to_owned),
        cursor,
        size: size.unwrap_or(32),
    };
    let mut request = Request::get("/admin/users").query("size", query.size);
    if let Some(keyword) = query.keyword {
        request = request.query("keyword", keyword);
    }
    if let Some(cursor) = query.cursor {
        request = request.query("cursor", cursor);
    }
    client.send_json(request).await
}

/// 按筛选条件查询后台任务。
pub async fn get_jobs(
    client: &Client,
    query: &AdminJobQuery,
) -> Result<CursorPage<AdminJobView, TimeIdCursor<String>>> {
    let mut request = Request::get("/admin/jobs").query("size", query.size);
    if let Some(kind) = query.kind {
        request = request.query("kind", kind);
    }
    if let Some(status) = query.status {
        request = request.query("status", status);
    }
    if let Some(cursor) = &query.cursor {
        request = request.query("cursor", cursor);
    }
    client.send_json(request).await
}

/// 启动后台任务。
pub async fn start_job(client: &Client, request: &StartAdminJobRequest) -> Result<AdminJobView> {
    client
        .send_json(Request::post("/admin/jobs").json(request))
        .await
}

/// 查询后台任务详情。
pub async fn get_job(client: &Client, job_id: &str) -> Result<AdminJobView> {
    client
        .send_json(Request::get(format!("/admin/jobs/{job_id}")))
        .await
}

/// 请求取消后台任务；实际停止会在安全检查点发生。
pub async fn cancel_job(client: &Client, job_id: &str) -> Result<AdminJobView> {
    client
        .send_json(Request::post(format!("/admin/jobs/{job_id}/cancel")))
        .await
}
