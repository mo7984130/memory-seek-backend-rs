use crate::config::AppConfig;
use crate::state::AppState;
use axum::Router;
use common::traits::controller::ControllerRouter;
use photo::PhotoState;
use std::sync::Arc;
use tracing::info;

/// 注册 Photo 模块路由
pub fn register(
    state: &Arc<AppState>,
    _cfg: &AppConfig,
) -> (Router<Arc<AppState>>, Router<Arc<AppState>>) {
    info!("注册 Photo 模块路由");

    // 构建 PhotoState
    let photo_state = Arc::new(PhotoState::new(
        state.db.clone(),
        state.redis.clone(),
        state.s3_client.clone(),
        state.token_cipher.clone(),
    ));

    // 获取路由
    let public_router = photo::Controller::public_routes().with_state(photo_state.clone());
    let protected_router = photo::Controller::protected_routes().with_state(photo_state);

    info!("Photo 模块路由注册成功");

    (public_router, protected_router)
}
