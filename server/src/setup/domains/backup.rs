use crate::util::MissDepError;
use crate::{config::AppConfig, setup::AppSetup};
use backup::BackupState;
use common::tokio::TaskManager;
use common::{Result, axum::controller_router::ControllerRouter};
use oss::S3Client;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{debug, info};

pub use backup::BackupConfig as Config;

/// 备份运行时资源，供其它域（如 photo/face-engine）按需获取。
pub struct BackupRuntime {
    #[allow(unused)]
    pub state: Arc<backup::BackupState>,
}

/// 注册备份管理接口。
#[common::register_async(
    slice = crate::setup::domains::APP_DOMAINS_FIRST,
    ty = crate::setup::InitFn,
)]
pub async fn register(config: &AppConfig, setup: &mut AppSetup) -> Result<()> {
    debug!("初始化 backup domain");

    let task_manager = setup
        .registry
        .get::<TaskManager>()
        .miss_dep("backup", "TaskManager")?
        .clone();

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
        task_manager,
    );
    let state = Arc::new(state);

    // 注册每日凌晨 6 点（本地时间）的定时备份任务
    backup::scheduler::register_scheduled(Arc::clone(&state));

    let protected_router =
        backup::controller::BackupController::protected_routes().with_state(Arc::clone(&state));
    setup.router.add_protected(protected_router);

    setup.registry.insert(BackupRuntime {
        state: Arc::clone(&state),
    });

    info!("初始化 backup domain 成功");

    Ok(())
}
