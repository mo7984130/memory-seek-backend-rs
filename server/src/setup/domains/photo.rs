use std::path::PathBuf;

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
    cfg: &AppConfig,
) -> (Router<Arc<AppState>>, Router<Arc<AppState>>) {
    info!("注册 Photo 模块路由");

    #[cfg(feature = "face-engine")]
    let backup_storage = {
        #[cfg(feature = "backup")]
        {
            Some(backup::storage::BackupStorage::new(
                PathBuf::from(&cfg.backup.local_path),
                state.s3_client.clone(),
                cfg.backup.s3_prefix.clone(),
            ))
        }
        #[cfg(not(feature = "backup"))]
        {
            None
        }
    };

    let photo_state = Arc::new(PhotoState::new(
        state.db.clone(),
        state.redis.clone(),
        state.s3_client.clone(),
        state.token_cipher.clone(),
        #[cfg(feature = "face-engine")]
        state.face_engine.clone(),
        #[cfg(feature = "face-engine")]
        backup_storage,
    ));

    // 获取路由
    let public_router = photo::Controller::public_routes().with_state(photo_state.clone());
    let protected_router = photo::Controller::protected_routes().with_state(photo_state);

    info!("Photo 模块路由注册成功");

    (public_router, protected_router)
}
