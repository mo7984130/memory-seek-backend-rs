use std::sync::Arc;
use axum::Router;
use auth::AuthState;
use common::traits::controller::ControllerRouter;
use crate::config::AppConfig;
use crate::state::AppState;

/// 注册 Auth 模块路由
pub fn register(state: &Arc<AppState>, cfg: &AppConfig) -> (Router<Arc<AppState>>, Router<Arc<AppState>>) {
    // 创建 EmailClient
    let email_client = email::EmailClient::new(
        &cfg.smtp.server,
        cfg.smtp.port,
        &cfg.smtp.username,
        &cfg.smtp.password,
        &cfg.smtp.from_email,
        &cfg.smtp.from_name,
    );

    // 构建 AuthState
    let auth_state = Arc::new(AuthState::new(
        state.db.clone(),
        state.redis.clone(),
        email_client,
        state.token_cipher.clone(),
    ));

    // 获取路由
    let public_router = auth::Controller::public_routes()
        .with_state(auth_state.clone());
    let protected_router = auth::Controller::protected_routes()
        .with_state(auth_state);

    (public_router, protected_router)
}
