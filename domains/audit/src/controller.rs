use std::sync::Arc;

use crate::{AuditState, service::AuditQueryer};
use axum::{Extension, Router, extract::State, routing::get};
use common::{
    Result,
    axum::{R, controller_router::ControllerRouter, ext::ToROkExt, extractors::ValidatedQuery},
    types::CursorPage,
};
use types::audit::{
    AuditId, AuditItem, AuditQuery, AuditStatsItem, AuditStatsQuery, AuditTopItem, AuditTopQuery,
};
use types::auth::user::{AdminId, UserId};
use types::cursor::TimeIdCursor;

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

impl AuditController {
    async fn stats(
        State(state): State<Arc<AuditState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<AuditStatsQuery>,
    ) -> Result<R<Vec<AuditStatsItem>>> {
        AdminId::new(user_id)?;
        AuditQueryer::query_stats(&state.db, &req).await.to_r_ok()
    }

    async fn top(
        State(state): State<Arc<AuditState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<AuditTopQuery>,
    ) -> Result<R<Vec<AuditTopItem>>> {
        AdminId::new(user_id)?;
        AuditQueryer::query_top(&state.db, &req).await.to_r_ok()
    }

    async fn audit(
        State(state): State<Arc<AuditState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<AuditQuery>,
    ) -> Result<R<CursorPage<AuditItem, TimeIdCursor<AuditId>>>> {
        AdminId::new(user_id)?;
        AuditQueryer::query_events(&state.db, &req).await.to_r_ok()
    }
}
