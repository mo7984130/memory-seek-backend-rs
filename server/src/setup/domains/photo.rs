use crate::config::AppConfig;
use crate::state::AppState;
use axum::Router;
use common::axum::controller_router::ControllerRouter;
use multi_level_cache::CacheConfig;
use photo::PhotoState;
use std::sync::Arc;
use tracing::info;

/// 注册 Photo 模块路由
pub fn register(
    state: &Arc<AppState>,
    _cfg: &AppConfig,
) -> (Router<Arc<AppState>>, Router<Arc<AppState>>) {
    info!("注册 Photo 模块路由");

    let photo_state = Arc::new(PhotoState::new(
        state.db.clone(),
        state.redis.clone(),
        CacheConfig::new(
            _cfg.cache.enabled,
            _cfg.cache.local_capacity,
            common::time::Duration::from_secs(_cfg.cache.local_ttl_secs),
        ),
        state.s3_client.clone(),
        #[cfg(feature = "face-engine")]
        state.face_engine.clone(),
        #[cfg(feature = "face-engine")]
        state.backup_state.clone(),
    ));

    // 获取路由
    let public_router = photo::Controller::public_routes().with_state(photo_state.clone());
    let protected_router = photo::Controller::protected_routes().with_state(photo_state);

    info!("Photo 模块路由注册成功");

    (public_router, protected_router)
}
