use common_core::Result;
use common_runtime::TaskManager;
use tracing::{debug, info};

use crate::{config::AppConfig, setup::AppSetup};

/// 初始化全局后台任务管理器，复用 `AppSetup.cancel_token` 作为统一取消源
#[common_macros::register_async(
    slice = crate::setup::bases::APP_BASES,
    ty = crate::setup::InitFn,
)]
pub async fn init(_config: &AppConfig, setup: &mut AppSetup) -> Result<()> {
    debug!("初始化 TaskManager");

    let task_manager = TaskManager::with_token(setup.cancel_token.clone());
    setup.registry.insert(task_manager);

    info!("TaskManager 初始化完成");
    Ok(())
}
