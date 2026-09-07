use crate::{config::AppConfig, setup::AppSetup, util::MissDepError};
use common::{Result, axum::controller_router::ControllerRouter};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{debug, info};
use user::UserState;

/// 注册 User 模块路由
#[common::register_async(
    slice = crate::setup::domains::APP_DOMAINS,
    ty = crate::setup::InitFn,
)]
pub async fn init(config: &AppConfig, setup: &mut AppSetup) -> Result<()> {
    debug!("初始化 User Domain");

    let register = &mut setup.registry;
    let router = &mut setup.router;

    // 构建 UserState
    let user_state = Arc::new(UserState::new(
        register
            .get::<DatabaseConnection>()
            .miss_dep("User", "DatabaseConnection")?
            .clone(),
        register
            .get::<common::Pool>()
            .miss_dep("User", "RedisPool")?
            .clone(),
        config.cache.to(),
        register
            .get::<oss::S3Client>()
            .miss_dep("User", "S3Client")?
            .clone(),
    ));

    // 获取路由
    let public_router = user::Controller::public_routes().with_state(user_state.clone());
    let protected_router = user::Controller::protected_routes().with_state(user_state);
    router.add_public(public_router);
    router.add_protected(protected_router);

    info!("初始化 User Domain 成功");

    Ok(())
}
