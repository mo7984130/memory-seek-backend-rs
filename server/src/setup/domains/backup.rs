use crate::config::AppConfig;
use crate::state::AppState;
use axum::Router;
use common::traits::controller::ControllerRouter;
use std::sync::Arc;
use tracing::info;

/// 注册 Backup 模块路由
pub fn register(
    state: &Arc<AppState>,
    _cfg: &AppConfig,
) -> (Router<Arc<AppState>>, Router<Arc<AppState>>) {
    info!("注册 Backup 模块路由");

    let backup_state = state.backup_state.clone().unwrap();

    let public_router =
        backup::controller::BackupController::public_routes().with_state(backup_state.clone());
    let protected_router =
        backup::controller::BackupController::protected_routes().with_state(backup_state);

    info!("Backup 模块路由注册成功");

    (public_router, protected_router)
}
