use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::{FromRef, State},
    routing::get,
};
use common::{
    Result, ext::ResultRExt, extractors::ValidatedQuery, models::CursorPage, r::R,
    traits::controller::ControllerRouter,
};
use types::auth::user::{AdminId, UserId};
use types::cursor::TimeIdCursor;
use types::photo::behavior::UserBehaviorId;
use types::photo::dto::behavior::{
    BehaviorAuditItem, BehaviorAuditQuery, BehaviorStatsItem, BehaviorStatsQuery, BehaviorTopItem,
    BehaviorTopQuery,
};

use crate::{AuditService, AuditState, BehaviorRecord};

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
        ValidatedQuery(req): ValidatedQuery<BehaviorStatsQuery>,
    ) -> Result<R<Vec<BehaviorStatsItem>>> {
        AdminId::new(user_id)?;
        let rows = AuditService::query_behavior_stats(&state.db, &req).await?;
        Ok(rows
            .into_iter()
            .map(|(bucket, count)| BehaviorStatsItem { bucket, count })
            .collect())
        .to_r_ok()
    }

    async fn top(
        State(state): State<AuditState>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<BehaviorTopQuery>,
    ) -> Result<R<Vec<BehaviorTopItem>>> {
        AdminId::new(user_id)?;
        let rows = AuditService::query_behavior_top(&state.db, &req).await?;
        Ok(rows
            .into_iter()
            .map(|(target_id, count)| BehaviorTopItem { target_id, count })
            .collect())
        .to_r_ok()
    }

    async fn audit(
        State(state): State<AuditState>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<BehaviorAuditQuery>,
    ) -> Result<R<CursorPage<BehaviorAuditItem, TimeIdCursor<UserBehaviorId>>>> {
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
