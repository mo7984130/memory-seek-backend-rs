use crate::{config::AppConfig, setup::AppSetup, util::MissDepError};
use auth::AuthState;
use common::{Result, axum::controller_router::ControllerRouter};
use email::EmailClient;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{debug, info};

/// 注册 Auth 模块路由
#[common::register_async(
    slice = crate::setup::domains::APP_DOMAINS,
    ty = crate::setup::InitFn,
)]
pub async fn init(_config: &AppConfig, setup: &mut AppSetup) -> Result<()> {
    debug!("初始化 Auth domain");

    let register = &mut setup.registry;
    let router = &mut setup.router;

    // 构建 AuthState
    let auth_state = Arc::new(AuthState::new(
        register
            .get::<DatabaseConnection>()
            .miss_dep("auth", "DatabaseConnection")?
            .clone(),
        register
            .get::<common::Pool>()
            .miss_dep("auth", "RedisPool")?
            .clone(),
        register
            .get::<EmailClient>()
            .miss_dep("auth", "Email Client")?
            .clone(),
    ));

    // 添加路由
    let public_router = auth::Controller::public_routes().with_state(auth_state.clone());
    let protected_router = auth::Controller::protected_routes().with_state(auth_state);
    router.add_public(public_router);
    router.add_protected(protected_router);

    info!("初始化 Auth domain 成功");

    Ok(())
}
