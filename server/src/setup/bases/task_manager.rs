use common::{Result, tokio::TaskManager};
use tracing::{debug, info};

use crate::{config::AppConfig, setup::AppSetup};

/// 初始化全局后台任务管理器。
///
/// 复用 `AppSetup.cancel_token` 作为根取消令牌，使所有注册进 TaskManager
/// 的任务与应用共享同一个取消源；应用关闭时由 shutdown 流程统一
/// `cancel_all() + wait_for_all()` 收尾。
#[common::register_async(
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
