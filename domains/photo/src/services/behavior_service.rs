use sea_orm::entity::prelude::Json;
use types::auth::user::{AdminId, UserId};
use types::cursor::TimeIdCursor;
use types::photo::behavior::{
    BehaviorRecord, BehaviorTargetType, UserBehaviorAction, UserBehaviorId,
};
use types::photo::dto::behavior::{
    BehaviorAuditItem, BehaviorAuditQuery, BehaviorStatsItem, BehaviorStatsQuery, BehaviorTopItem,
    BehaviorTopQuery,
};

use crate::state::PhotoState;
use common::models::CursorPage;
use common::{
    Result, inc_error, metrics_group, metrics_name, metrics_success, utils::MetricsTimerExt,
};

/// 行为审计记录请求（只追加，不删除）
#[derive(Clone, Debug)]
pub(crate) struct BehaviorRecordReq {
    pub user_id: UserId,
    pub action: UserBehaviorAction,
    pub target_type: Option<BehaviorTargetType>,
    pub target_id: Option<i64>,
    pub detail: Option<Json>,
    pub ip: Option<String>,
}

impl BehaviorRecordReq {
    pub(crate) fn new(user_id: UserId, action: UserBehaviorAction) -> Self {
        Self {
            user_id,
            action,
            target_type: None,
            target_id: None,
            detail: None,
            ip: None,
        }
    }

    pub(crate) fn with_photo(mut self, photo_id: i64) -> Self {
        self.target_type = Some(BehaviorTargetType::Photo);
        self.target_id = Some(photo_id);
        self
    }

    pub(crate) fn with_target(mut self, target_type: BehaviorTargetType, target_id: i64) -> Self {
        self.target_type = Some(target_type);
        self.target_id = Some(target_id);
        self
    }

    pub(crate) fn with_detail(mut self, detail: Json) -> Self {
        self.detail = Some(detail);
        self
    }

    pub(crate) fn with_ip(mut self, ip: Option<String>) -> Self {
        self.ip = ip;
        self
    }
}

pub(crate) struct BehaviorService;

impl BehaviorService {
    /// 同步写入行为审计记录。
    ///
    /// 审计写入失败不阻断业务，仅记录 warning；只追加不删除。
    #[tracing::instrument(
        skip_all,
        fields(user_id = %req.user_id, action = %req.action.as_str())
    )]
    pub async fn record(state: &PhotoState, req: BehaviorRecordReq) {
        metrics_group!();
        if let Err(e) = state
            .repo
            .record_behavior(
                req.user_id,
                req.action,
                req.target_type,
                req.target_id,
                req.detail,
                req.ip.as_deref(),
            )
            .await
        {
            inc_error!("db");
            common::caller_warn!(error = ?e, action = %req.action.as_str(), "record_behavior_failed");
        } else {
            metrics_success!();
        }
    }

    /// 异步记录照片浏览（图片访问热路径）：token 内嵌浏览者身份，按 file_id 反查照片 ID 后写入。
    ///
    /// 图片访问不依赖登录态，浏览者身份来自签发给图片访问者的 token。
    #[tracing::instrument(skip_all, fields(viewer_id = %viewer_id, file_id = %file_id))]
    pub fn record_view_async(
        state: &PhotoState,
        viewer_id: UserId,
        file_id: String,
        ip: Option<String>,
    ) {
        state.repo.record_photo_view_async(viewer_id, file_id, ip);
    }
}

// 管理端统计
impl BehaviorService {
    #[common::metered]
    #[tracing::instrument(skip_all, fields(admin_user_id = %admin))]
    pub async fn get_stats(
        state: &PhotoState,
        admin: AdminId,
        req: BehaviorStatsQuery,
    ) -> Result<Vec<BehaviorStatsItem>> {
        let rows = state
            .repo
            .query_behavior_stats(&req)
            .timed(metrics_name!("query_stats"))
            .await?;

        Ok(rows
            .into_iter()
            .map(|(bucket, count)| BehaviorStatsItem { bucket, count })
            .collect())
    }

    #[common::metered]
    #[tracing::instrument(skip_all, fields(admin_user_id = %admin))]
    pub async fn get_top(
        state: &PhotoState,
        admin: AdminId,
        req: BehaviorTopQuery,
    ) -> Result<Vec<BehaviorTopItem>> {
        let rows = state
            .repo
            .query_behavior_top(&req)
            .timed(metrics_name!("query_top"))
            .await?;

        Ok(rows
            .into_iter()
            .map(|(target_id, count)| BehaviorTopItem { target_id, count })
            .collect())
    }

    #[common::metered]
    #[tracing::instrument(skip_all, fields(admin_user_id = %admin))]
    pub async fn get_audit(
        state: &PhotoState,
        admin: AdminId,
        req: BehaviorAuditQuery,
    ) -> Result<CursorPage<BehaviorAuditItem, String>> {
        let page = state
            .repo
            .query_behavior_audit(&req)
            .timed(metrics_name!("query_audit"))
            .await?;

        let page = page.with_next_cursor(|record: &BehaviorRecord| {
            Ok(TimeIdCursor::<UserBehaviorId> {
                created_at: record.created_at,
                id: record.id,
            }
            .encode())
        })?;

        Ok(page.map_records(|records| records.into_iter().map(to_audit_item).collect()))
    }
}

fn to_audit_item(record: BehaviorRecord) -> BehaviorAuditItem {
    BehaviorAuditItem {
        id: record.id,
        user_id: record.user_id,
        action: record.action,
        target_type: record.target_type,
        target_id: record.target_id,
        detail: record.detail,
        ip: record.ip,
        created_at: record.created_at,
    }
}
