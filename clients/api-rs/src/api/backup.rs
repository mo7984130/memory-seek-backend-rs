//! 备份相关 API(管理端)。
//!
//! 路由前缀:`/admin/backup`(需鉴权,且仅管理员可访问)。

use crate::client::{Client, Request};
use crate::error::Result;
use types_admin::job::AdminJobView;
use types_backup::RestoreRequest;

/// 触发一次定时备份流程(含过期备份清理)。`POST /admin/backup/trigger`
pub async fn trigger_backup(client: &Client) -> Result<AdminJobView> {
    client
        .send_json(Request::post("/admin/backup/trigger"))
        .await
}

/// 触发一次管理员手动备份。`POST /admin/backup/manual`
pub async fn manual_backup(client: &Client) -> Result<AdminJobView> {
    client
        .send_json(Request::post("/admin/backup/manual"))
        .await
}

/// 按指定备份源与恢复层级入队一次数据恢复。`POST /admin/backup/restore`
///
/// `RestoreRequest::confirm_run_id` 必须与 `run_id` 一致,用于二次确认高危操作。
/// 恢复是后台任务,进度与结果通过返回的 `AdminJobView` 查询。
pub async fn restore_backup(client: &Client, request: &RestoreRequest) -> Result<AdminJobView> {
    client
        .send_json(Request::post("/admin/backup/restore").json(request))
        .await
}
