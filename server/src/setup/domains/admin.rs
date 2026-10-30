use std::sync::Arc;

use crate::setup::domains::backup::BackupRuntime;
use crate::{setup::AppSetup, util::MissDepError};
use common_core::Result;
use common_runtime::TaskManager;
use common_web::controller_router::ControllerRouter;
use sea_orm::DatabaseConnection;
use user::UserState;
use visual::VisualState;

/// 在各业务域初始化完成后，统一挂载管理端路由。
pub async fn init(_config: &crate::config::AppConfig, setup: &mut AppSetup) -> Result<()> {
    let user_state = setup
        .registry
        .get::<Arc<UserState>>()
        .miss_dep("Admin", "UserState")?
        .clone();
    let visual_state = setup
        .registry
        .get::<Arc<VisualState>>()
        .miss_dep("Admin", "VisualState")?
        .clone();
    let backup_state = setup
        .registry
        .get::<BackupRuntime>()
        .miss_dep("Admin", "BackupRuntime")?
        .state
        .clone();
    let db = setup
        .registry
        .get::<DatabaseConnection>()
        .miss_dep("Admin", "DatabaseConnection")?
        .clone();
    let task_manager = setup
        .registry
        .get::<TaskManager>()
        .miss_dep("Admin", "TaskManager")?
        .clone();

    admin::tasks::prepare_job_store(&db).await?;
    let state = Arc::new(::admin::AdminState::new(
        db,
        user_state,
        visual_state,
        backup_state,
        task_manager,
    ));
    let protected = ::admin::Controller::protected_routes().with_state(state);
    setup.router.add_protected(protected);
    Ok(())
}
