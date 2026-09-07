use crate::util::MissDepError;
use crate::{config::AppConfig, setup::AppSetup};
use backup::BackupState;
use common::error::{AppError, contextual::ext::ResultContextualExt};
use common::{Result, axum::controller_router::ControllerRouter};
use oss::S3Client;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{debug, info};

pub use backup::BackupConfig as Config;

pub struct BackupRuntime {
    #[allow(unused)]
    pub state: Arc<backup::BackupState>,
    #[allow(unused)]
    pub scheduler: backup::BackupScheduler,
}

/// 注册备份管理接口。
#[common::register_async(
    slice = crate::setup::domains::APP_DOMAINS_FIRST,
    ty = crate::setup::InitFn,
)]
pub async fn register(config: &AppConfig, setup: &mut AppSetup) -> Result<()> {
    debug!("初始化 backup domain");

    let state = BackupState::new(
        setup
            .registry
            .get::<DatabaseConnection>()
            .miss_dep("backup", "DatabaseConnection")?
            .clone(),
        setup
            .registry
            .get::<S3Client>()
            .miss_dep("backup", "S3 Client")?
            .clone(),
        config.backup.clone(),
    );
    let state = Arc::new(state);

    let scheduler = backup::BackupScheduler::new(Arc::clone(&state))
        .await
        .context_err(
            "backup_init_err",
            "备份调度器初始化失败",
            AppError::InternalServerError,
        )?;
    scheduler.start().await.context_err(
        "backup_start_err",
        "备份调度器启动失败",
        AppError::InternalServerError,
    )?;

    let runtime = BackupRuntime {
        state: Arc::clone(&state),
        scheduler: scheduler.clone(),
    };
    setup.registry.insert(runtime);

    let protected_router =
        backup::controller::BackupController::protected_routes().with_state(Arc::clone(&state));
    setup.router.add_protected(protected_router);

    info!("初始化 backup domain 成功");

    Ok(())
}
