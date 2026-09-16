#[cfg(feature = "face-engine")]
use crate::setup::domains::backup::BackupRuntime;
use crate::{config::AppConfig, setup::AppSetup, util::MissDepError};
use common::{Result, axum::controller_router::ControllerRouter};
use visual::VisualState;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{debug, info};

/// 注册 Visual 模块路由
#[common::register_async(
    slice = crate::setup::domains::APP_DOMAINS,
    ty = crate::setup::InitFn,
)]
pub async fn init(config: &AppConfig, setup: &mut AppSetup) -> Result<()> {
    debug!("初始化 visual domain");

    let register = &mut setup.registry;
    let router = &mut setup.router;

    let visual_state = Arc::new(VisualState::new(
        register
            .get::<DatabaseConnection>()
            .miss_dep("Visual", "DatabaseConnection")?
            .clone(),
        register
            .get::<common::Pool>()
            .miss_dep("Visual", "RedisPool")?
            .clone(),
        config.cache.to(),
        register
            .get::<oss::S3Client>()
            .miss_dep("Visual", "S3Client")?
            .clone(),
        #[cfg(feature = "face-engine")]
        register
            .get::<Arc<insight_face_rs::FaceEngine>>()
            .miss_dep("Visual", "FaceEngine")?
            .clone(),
        #[cfg(feature = "face-engine")]
        register
            .get::<BackupRuntime>()
            .miss_dep("Visual", "BackupRuntime")?
            .state
            .clone(),
    ));

    // 获取路由
    let public_router = visual::Controller::public_routes().with_state(visual_state.clone());
    let protected_router = visual::Controller::protected_routes().with_state(visual_state);
    router.add_public(public_router);
    router.add_protected(protected_router);

    info!("初始化 Visual Domain 成功");

    Ok(())
}
