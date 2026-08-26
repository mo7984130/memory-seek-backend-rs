use crate::config::AppConfig;
use crate::state::AppState;
use audit::{AuditController, AuditState};
use axum::Router;
use common::traits::controller::ControllerRouter;
use std::sync::Arc;

/// 注册审计管理路由。
pub fn register(
    state: &Arc<AppState>,
    _cfg: &AppConfig,
) -> (Router<Arc<AppState>>, Router<Arc<AppState>>) {
    let audit_state = Arc::new(AuditState {
        db: state.db.clone(),
    });
    let public_router = Router::new();
    let protected_router = Router::new().nest(
        "/photo/admin/behaviors",
        AuditController::protected_routes().with_state(audit_state),
    );
    (public_router, protected_router)
}
