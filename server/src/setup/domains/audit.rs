use crate::{config::AppConfig, setup::AppSetup, util::MissDepError};
use audit::{AuditController, AuditState};
use axum::Router;
use common::{Result, axum::controller_router::ControllerRouter};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{debug, info};

#[common::register_async(
    slice = crate::setup::domains::APP_DOMAINS,
    ty = crate::setup::InitFn,
)]
pub async fn init(_config: &AppConfig, setup: &mut AppSetup) -> Result<()> {
    debug!("初始化 Audit domain");

    let register = &mut setup.registry;
    let router = &mut setup.router;

    let audit_state = Arc::new(AuditState {
        db: register
            .get::<DatabaseConnection>()
            .miss_dep("audit", "DatabaseConnection")?
            .clone(),
    });
    let protected_router = Router::new().nest(
        "/admin/audits",
        AuditController::protected_routes().with_state(audit_state),
    );
    router.add_protected(protected_router);

    info!("初始化 Audit domain 成功");
    Ok(())
}
