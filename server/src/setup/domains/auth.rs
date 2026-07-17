use crate::state::AppState;
use auth::AuthState;
use axum::Router;
use common::traits::controller::ControllerRouter;
use std::sync::Arc;
use tracing::info;

/// 注册 Auth 模块路由
pub fn register(
    state: &Arc<AppState>,
    _cfg: &crate::config::AppConfig,
) -> (Router<Arc<AppState>>, Router<Arc<AppState>>) {
    info!("注册 Auth 模块路由");

    // 构建 AuthState
    let auth_state = Arc::new(AuthState::new(
        state.db.clone(),
        state.redis.clone(),
        state.email_client.clone(),
        state.token_cipher.clone(),
    ));

    // 获取路由
    let public_router = auth::Controller::public_routes().with_state(auth_state.clone());
    let protected_router = auth::Controller::protected_routes().with_state(auth_state);

    info!("Auth 模块路由注册成功");

    (public_router, protected_router)
}
