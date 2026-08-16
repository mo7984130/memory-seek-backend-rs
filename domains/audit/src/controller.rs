use std::sync::Arc;

use crate::{AuditService, AuditState};
use axum::{
    Extension, Router,
    extract::{FromRef, State},
    routing::get,
};
use common::{
    Result, ext::ResultRExt, extractors::ValidatedQuery, models::CursorPage, r::R,
    traits::controller::ControllerRouter,
};
use types::audit::{
    AuditItem, AuditQuery, AuditStatsItem, AuditStatsQuery, AuditTopItem, AuditTopQuery,
    BehaviorRecord,
};
use types::auth::user::{AdminId, UserId};
use types::cursor::TimeIdCursor;
use types::photo::behavior::UserBehaviorId;

pub struct AuditController;

impl ControllerRouter for AuditController {
    type State = AuditState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::<Arc<Self::State>>::new()
            .route("/stats", get(Self::stats))
            .route("/top", get(Self::top))
            .route("/audit", get(Self::audit))
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::<Arc<Self::State>>::new()
    }
}

impl FromRef<Arc<AuditState>> for AuditState {
    fn from_ref(state: &Arc<AuditState>) -> Self {
        state.as_ref().clone()
    }
}

impl AuditController {
    async fn stats(
        State(state): State<AuditState>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<AuditStatsQuery>,
    ) -> Result<R<Vec<AuditStatsItem>>> {
        AdminId::new(user_id)?;
        let rows = AuditService::query_behavior_stats(&state.db, &req).await?;
        Ok(rows
            .into_iter()
            .map(|(bucket, count)| AuditStatsItem { bucket, count })
            .collect())
        .to_r_ok()
    }

    async fn top(
        State(state): State<AuditState>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<AuditTopQuery>,
    ) -> Result<R<Vec<AuditTopItem>>> {
        AdminId::new(user_id)?;
        let rows = AuditService::query_behavior_top(&state.db, &req).await?;
        Ok(rows
            .into_iter()
            .map(|(target_id, count)| AuditTopItem { target_id, count })
            .collect())
        .to_r_ok()
    }

    async fn audit(
        State(state): State<AuditState>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<AuditQuery>,
    ) -> Result<R<CursorPage<AuditItem, TimeIdCursor<UserBehaviorId>>>> {
        AdminId::new(user_id)?;
        let records = AuditService::query_behavior_audit(&state.db, &req).await?;
        let page = CursorPage::from_oversize(records, req.size);
        let page = page.with_next_cursor(|record: &BehaviorRecord| {
            Ok(TimeIdCursor::<UserBehaviorId> {
                created_at: record.created_at,
                id: record.id,
            })
        })?;
        Ok(page.map_records(|records| records.into_iter().map(to_audit_item).collect())).to_r_ok()
    }
}

fn to_audit_item(record: BehaviorRecord) -> AuditItem {
    AuditItem {
        id: record.id,
        user_id: record.user_id,
        action: record.action,
        target_type: record.target_type,
        target_id: record.target_id,
        detail: record.detail,
        created_at: record.created_at,
    }
}
