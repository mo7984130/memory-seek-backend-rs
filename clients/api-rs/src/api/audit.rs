//! 行为审计相关 API(管理端)。
//!
//! 路由前缀:`/admin/audits`(需鉴权,且仅管理员可访问)。

use crate::client::{Client, Request};
use crate::error::Result;
use common_core::time::DateTime;
use common_core::types::CursorPage;
use types_audit::{AuditGranularity, AuditItem, AuditStatsItem, AuditTopItem};
use types_core::cursor::TimeIdCursor;
use types_core::{AuditId, UserId};

/// 审计量时序统计(按日/周/月聚合)。`GET /admin/audits/stats`
pub async fn get_audit_stats(
    client: &Client,
    event_type: Option<String>,
    target_type: Option<String>,
    start: Option<DateTime>,
    end: Option<DateTime>,
    granularity: Option<AuditGranularity>,
) -> Result<Vec<AuditStatsItem>> {
    let mut request = Request::get("/admin/audits/stats");
    if let Some(value) = event_type {
        request = request.query("eventType", value);
    }
    if let Some(value) = target_type {
        request = request.query("targetType", value);
    }
    if let Some(value) = start {
        request = request.query("start", value);
    }
    if let Some(value) = end {
        request = request.query("end", value);
    }
    if let Some(value) = granularity {
        request = request.query("granularity", value);
    }
    client.send_json(request).await
}

/// 热门目标排行(如浏览量 Top N 影像)。`GET /admin/audits/top`
pub async fn get_audit_top(
    client: &Client,
    event_type: Option<String>,
    target_type: Option<String>,
    limit: Option<u64>,
) -> Result<Vec<AuditTopItem>> {
    let mut request = Request::get("/admin/audits/top");
    if let Some(value) = event_type {
        request = request.query("eventType", value);
    }
    if let Some(value) = target_type {
        request = request.query("targetType", value);
    }
    if let Some(value) = limit {
        request = request.query("limit", value);
    }
    client.send_json(request).await
}

/// 审计流水(可按照片/人物/动作追溯)。`GET /admin/audits/audit`
pub async fn get_audit_items(
    client: &Client,
    event_type: Option<String>,
    target_type: Option<String>,
    target_id: Option<i64>,
    actor_id: Option<UserId>,
    cursor: Option<TimeIdCursor<AuditId>>,
    size: Option<u64>,
) -> Result<CursorPage<AuditItem, TimeIdCursor<AuditId>>> {
    let mut request = Request::get("/admin/audits/audit");
    if let Some(value) = event_type {
        request = request.query("eventType", value);
    }
    if let Some(value) = target_type {
        request = request.query("targetType", value);
    }
    if let Some(value) = target_id {
        request = request.query("targetId", value);
    }
    if let Some(value) = actor_id {
        request = request.query("actorId", value);
    }
    if let Some(value) = cursor {
        request = request.query("cursor", value);
    }
    if let Some(value) = size {
        request = request.query("size", value);
    }
    client.send_json(request).await
}
