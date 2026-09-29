#[cfg(feature = "face-engine")]
use crate::setup::domains::backup::BackupRuntime;
use crate::{config::AppConfig, setup::AppSetup, util::MissDepError};
use common_core::{AppError, ContextualError, Result};
use common_runtime::TaskManager;
use common_web::controller_router::ControllerRouter;
use sea_orm::DatabaseConnection;
#[cfg(feature = "face-engine")]
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, info};
use visual::VisualState;

/// 影像域配置(由 `visual` crate 定义, 在 server 配置中重导出)
pub use visual::VisualConfig as Config;

/// 注册 Visual 模块路由
#[common_macros::register_async(
    slice = crate::setup::domains::APP_DOMAINS,
    ty = crate::setup::InitFn,
)]
pub async fn init(config: &AppConfig, setup: &mut AppSetup) -> Result<()> {
    debug!("初始化 visual domain");

    // 转码配置校验 + ffmpeg 可用性探测: 缺 ffmpeg/配置非法时尽快暴露,
    // 而非等到运行时让每个视频静默 failed。
    validate_transcode(config).await?;

    let register = &mut setup.registry;
    let router = &mut setup.router;

    let visual_state = VisualState::new(
        register
            .get::<DatabaseConnection>()
            .miss_dep("Visual", "DatabaseConnection")?
            .clone(),
        register
            .get::<common_redis::Pool>()
            .miss_dep("Visual", "RedisPool")?
            .clone(),
        config.cache.to(),
        register
            .get::<oss::S3Client>()
            .miss_dep("Visual", "S3Client")?
            .clone(),
        config.server.tmp_path.clone(),
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
        register
            .get::<TaskManager>()
            .miss_dep("Visual", "TaskManager")?
            .clone(),
        config.visual.clone(),
    );

    // 获取路由
    let public_router = visual::Controller::public_routes().with_state(visual_state.clone());
    let protected_router = visual::Controller::protected_routes().with_state(visual_state);
    router.add_public(public_router);
    router.add_protected(protected_router);

    info!("初始化 Visual Domain 成功");

    Ok(())
}

/// 校验视频转码配置并探测 ffmpeg 可用性。
///
/// `transcode.enabled` 时若配置非法或 ffmpeg 不可执行, 直接启动失败(fail-fast)。
async fn validate_transcode(config: &AppConfig) -> Result<()> {
    let transcode = &config.visual.transcode;

    if let Err(reason) = transcode.validate() {
        return Err(ContextualError::warn_without_source(
            "visual_transcode_config_invalid",
            format!("视频转码配置非法: {reason}"),
            AppError::InternalServerError,
        )
        .emit());
    }

    if !transcode.enabled {
        return Ok(());
    }

    match timeout(
        Duration::from_secs(5),
        Command::new(&transcode.ffmpeg_path)
            .arg("-version")
            .output(),
    )
    .await
    {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => Err(ContextualError::warn_without_source(
            "visual_ffmpeg_unavailable",
            format!(
                "ffmpeg 执行失败(退出码 {:?}), 请检查 transcode.ffmpeg_path",
                output.status.code()
            ),
            AppError::InternalServerError,
        )
        .emit()),
        Ok(Err(error)) => Err(ContextualError::error(
            "visual_ffmpeg_unavailable",
            "无法执行 ffmpeg, 请确认已安装并在 PATH(或配置 transcode.ffmpeg_path)",
            error,
            AppError::InternalServerError,
        )
        .emit()),
        Err(_elapsed) => Err(ContextualError::warn_without_source(
            "visual_ffmpeg_timeout",
            "ffmpeg 可用性探测超时",
            AppError::InternalServerError,
        )
        .emit()),
    }
}
