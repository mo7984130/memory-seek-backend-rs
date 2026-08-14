use common::models::CursorPage;
use sea_orm::entity::prelude::Json;
use types::auth::user::UserId;
use types::photo::behavior::{BehaviorRecord, BehaviorTargetType, UserBehaviorAction};
use types::photo::dto::behavior::{BehaviorAuditQuery, BehaviorStatsQuery, BehaviorTopQuery};

use super::PhotoRepo;
use crate::mappers::{behavior_mapper::BehaviorMapper, photo_mapper::PhotoMapper};

impl PhotoRepo {
    /// 持久化行为审计记录, 写入失败由调用方决定是否忽略.
    pub(crate) async fn record_behavior(
        &self,
        user_id: UserId,
        action: UserBehaviorAction,
        target_type: Option<BehaviorTargetType>,
        target_id: Option<i64>,
        detail: Option<Json>,
        ip: Option<&str>,
    ) -> common::error::contextual::Result<()> {
        BehaviorMapper::insert(
            &self.db,
            user_id,
            action,
            target_type,
            target_id,
            detail,
            ip,
        )
        .await
    }

    /// 异步记录公开图片访问行为, 不阻塞图片响应.
    pub(crate) fn record_photo_view_async(
        &self,
        viewer_id: UserId,
        file_id: String,
        ip: Option<String>,
    ) {
        let db = self.db.clone();
        tokio::spawn(async move {
            let photo_id = match PhotoMapper::query_photo_id_by_file_id(&db, &file_id).await {
                Ok(Some(photo_id)) => photo_id,
                Ok(None) => return,
                Err(_) => return,
            };
            let _ = BehaviorMapper::insert(
                &db,
                viewer_id,
                UserBehaviorAction::View,
                Some(BehaviorTargetType::Photo),
                Some(photo_id.0),
                None,
                ip.as_deref(),
            )
            .await;
        });
    }

    /// 查询按时间桶聚合的行为统计.
    pub(crate) async fn query_behavior_stats(
        &self,
        req: &BehaviorStatsQuery,
    ) -> common::error::contextual::Result<Vec<(chrono::DateTime<chrono::Utc>, i64)>> {
        BehaviorMapper::query_stats(
            &self.db,
            req.action,
            req.target_type,
            req.start,
            req.end,
            req.granularity.as_trunc(),
        )
        .await
    }

    /// 查询行为目标访问排名.
    pub(crate) async fn query_behavior_top(
        &self,
        req: &BehaviorTopQuery,
    ) -> common::error::contextual::Result<Vec<(i64, i64)>> {
        BehaviorMapper::query_top_targets(&self.db, req.action, req.target_type, req.limit).await
    }

    /// 按游标查询行为审计明细.
    pub(crate) async fn query_behavior_audit(
        &self,
        req: &BehaviorAuditQuery,
    ) -> common::error::contextual::Result<CursorPage<BehaviorRecord, ()>> {
        Ok(CursorPage::from_oversize(
            BehaviorMapper::query_audit_page(
                &self.db,
                req.action,
                req.target_type,
                req.target_id,
                req.user_id,
                &req.cursor,
                req.size,
            )
            .await?,
            req.size,
        ))
    }
}
