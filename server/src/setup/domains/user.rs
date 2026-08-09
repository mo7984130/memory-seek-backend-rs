use crate::config::AppConfig;
use crate::state::AppState;
use axum::Router;
use common::traits::controller::ControllerRouter;
use std::sync::Arc;
use tracing::info;
use user::UserState;

/// 注册 User 模块路由
pub fn register(
    state: &Arc<AppState>,
    cfg: &AppConfig,
) -> (Router<Arc<AppState>>, Router<Arc<AppState>>) {
    info!("注册 User 模块路由");

    // 构建 UserState
    let user_state = Arc::new(UserState::new(
        state.db.clone(),
        state.redis.clone(),
        common::cache::CacheConfig::new(
            cfg.cache.enabled,
            cfg.cache.local_capacity,
            std::time::Duration::from_secs(cfg.cache.local_ttl_secs),
        ),
        state.s3_client.clone(),
    ));

    // 获取路由
    let public_router = user::Controller::public_routes().with_state(user_state.clone());
    let protected_router = user::Controller::protected_routes().with_state(user_state);

    info!("User 模块路由注册成功");

    (public_router, protected_router)
}
