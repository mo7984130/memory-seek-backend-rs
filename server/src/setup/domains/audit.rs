use crate::{config::AppConfig, setup::AppSetup};
use common_core::Result;
use tracing::{debug, info};

/// 初始化审计域并挂载管理员审计路由
#[common_macros::register_async(
    slice = crate::setup::domains::APP_DOMAINS,
    ty = crate::setup::InitFn,
)]
pub async fn init(_config: &AppConfig, setup: &mut AppSetup) -> Result<()> {
    debug!("初始化 Audit domain");

    let _ = setup;
    info!("初始化 Audit domain 成功");
    Ok(())
}
