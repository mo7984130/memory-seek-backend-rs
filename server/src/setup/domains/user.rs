use std::sync::Arc;
use axum::Router;
use user::UserState;
use common::traits::controller::ControllerRouter;
use crate::config::AppConfig;
use crate::state::AppState;

/// 注册 User 模块路由
pub fn register(state: &Arc<AppState>, _cfg: &AppConfig) -> (Router<Arc<AppState>>, Router<Arc<AppState>>) {
    // 构建 UserState
    let user_state = Arc::new(UserState::new(
        state.db.clone(),
        state.redis.clone(),
        state.s3_client.clone(),
        state.token_cipher.clone(),
    ));

    // 获取路由
    let public_router = user::Controller::public_routes()
        .with_state(user_state.clone());
    let protected_router = user::Controller::protected_routes()
        .with_state(user_state);

    (public_router, protected_router)
}
