//! 统一临时文件目录: 创建、规范化为绝对路径并登记到 registry。
//!
//! 上传落盘、视频转码输入/输出等均复用该目录; 解析为绝对路径后, 日志展示与
//! 下游创建/清理临时文件统一使用完整路径, 避免受进程 CWD 影响产生歧义。

use std::path::PathBuf;

use common_core::error::contextual::ext::{IntoContextualExt, ResultContextualExt};
use common_core::{AppError, Result};
use tracing::{debug, info};

use crate::{config::AppConfig, setup::AppSetup};

/// 统一临时文件目录(已解析为绝对路径)。
#[derive(Debug, Clone)]
pub struct TmpDir(pub PathBuf);

/// 创建临时文件目录并登记为 [`TmpDir`]。
#[common_macros::register_async(
    slice = crate::setup::bases::APP_BASES,
    ty = crate::setup::InitFn,
)]
pub async fn init(config: &AppConfig, setup: &mut AppSetup) -> Result<()> {
    debug!("初始化临时文件目录");

    let tmp_path = &config.server.tmp_path;
    std::fs::create_dir_all(tmp_path)
        .into_contextual()
        .context_err(
            "create_tmp_dir_failed",
            "创建临时目录失败",
            AppError::InternalServerError,
        )?;
    // 规范化为绝对路径: 使日志展示与下游创建/清理临时文件统一使用完整路径
    let absolute = std::path::absolute(tmp_path)
        .into_contextual()
        .context_err(
            "resolve_tmp_dir_failed",
            "解析临时目录绝对路径失败",
            AppError::InternalServerError,
        )?;
    info!(path = %absolute.display(), "临时文件目录已就绪");

    setup.registry.insert(TmpDir(absolute));
    Ok(())
}
