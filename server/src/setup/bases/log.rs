use std::path::PathBuf;

use serde::Deserialize;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    EnvFilter, Registry,
    fmt::{self},
    prelude::*,
};

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default = "default_log_dir")]
    log_dir: PathBuf,
    #[serde(default = "default_log_file_name")]
    log_file_name: String,
}

fn default_log_dir() -> PathBuf {
    PathBuf::from("./logs")
}
fn default_log_file_name() -> String {
    "app.log".to_string()
}

/// 初始化完整日志系统（stdout + 文件）
///
/// 如果全局 subscriber 已被 main 中的 bootstrap subscriber 占用，
/// 则跳过文件日志初始化，返回 None。此时 bootstrap subscriber 继续工作。
pub fn init(cfg: &Config) -> Option<WorkerGuard> {
    // 日志输出
    // 每天创建一个新文件
    let file_appender = tracing_appender::rolling::daily(&cfg.log_dir, &cfg.log_file_name);
    // 异步写入器
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // 控制台layer
    let stdout_layer = fmt::layer().with_ansi(true).with_writer(std::io::stdout);
    // 日志文件layer
    let file_layer = fmt::layer().with_ansi(false).with_writer(non_blocking);

    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let registry = Registry::default()
        .with(EnvFilter::new(format!("{},sqlx=warn", log_level)))
        .with(stdout_layer)
        .with(file_layer);

    #[cfg(feature = "metrics")]
    let registry = {
        use metrics_tracing_context::MetricsLayer;
        registry.with(MetricsLayer::new())
    };

    match registry.try_init() {
        Ok(()) => {
            tracing::info!("日志系统初始化完成");
            Some(guard)
        }
        Err(_) => {
            // bootstrap subscriber 已存在，丢弃文件写入器避免无用线程
            drop(guard);
            tracing::warn!("日志系统已在启动阶段初始化，跳过文件日志配置");
            None
        }
    }
}
