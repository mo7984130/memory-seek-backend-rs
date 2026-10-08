use std::sync::Arc;

use crate::{AuditState, service::AuditQueryer};
use axum::{Extension, Router, extract::State, routing::get};
use common_core::{Result, types::CursorPage};
use common_web::{
    R, controller_router::ControllerRouter, ext::ToROkExt, extractors::ValidatedQuery,
};
use types_audit::{
    AuditId, AuditItem, AuditQuery, AuditStatsItem, AuditStatsQuery, AuditTopItem, AuditTopQuery,
};
use types_core::cursor::TimeIdCursor;
use types_core::{AdminId, UserId};

pub struct AuditController;

impl ControllerRouter for AuditController {
    type State = AuditState;

    /// 审计模块的受保护路由，仅管理员可访问
    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::<Arc<Self::State>>::new()
            .route("/stats", get(Self::stats))
            .route("/top", get(Self::top))
            .route("/audit", get(Self::audit))
    }

    /// 审计模块无公开路由
    fn public_routes() -> Router<Arc<Self::State>> {
        Router::<Arc<Self::State>>::new()
    }
}

impl AuditController {
    /// 查询审计事件统计（按时间分桶）
    async fn stats(
        State(state): State<Arc<AuditState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<AuditStatsQuery>,
    ) -> Result<R<Vec<AuditStatsItem>>> {
        AdminId::new(user_id)?;
        AuditQueryer::query_stats(&state.db, &req).await.to_r_ok()
    }

    /// 查询事件数量最多的目标排行
    async fn top(
        State(state): State<Arc<AuditState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<AuditTopQuery>,
    ) -> Result<R<Vec<AuditTopItem>>> {
        AdminId::new(user_id)?;
        AuditQueryer::query_top(&state.db, &req).await.to_r_ok()
    }

    /// 游标分页查询审计事件列表
    async fn audit(
        State(state): State<Arc<AuditState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<AuditQuery>,
    ) -> Result<R<CursorPage<AuditItem, TimeIdCursor<AuditId>>>> {
        AdminId::new(user_id)?;
        AuditQueryer::query_events(&state.db, &req).await.to_r_ok()
    }
}
