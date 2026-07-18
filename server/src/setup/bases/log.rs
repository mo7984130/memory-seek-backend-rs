use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    EnvFilter, Registry,
    fmt::{self},
    prelude::*,
};

/// 初始化日志系统（stdout + 文件滚动写入），在 main 入口处尽早调用。
///
/// 参数 `cli_log_dir` 和 `cli_log_file` 为 CLI 传入值（优先级最高），
/// 未提供时回退到环境变量 `MEMORY_SEEK_LOG_DIR` / `MEMORY_SEEK_LOG_FILE`，
/// 最终默认 `/var/log/memory-seek-server` / `app.log`。
pub fn init(cli_log_dir: Option<String>, cli_log_file: Option<String>) -> Option<WorkerGuard> {
    let log_dir = cli_log_dir
        .map(PathBuf::from)
        .or_else(|| std::env::var("MEMORY_SEEK_LOG_DIR").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/var/log/memory-seek-server"));
    let log_file_name = cli_log_file
        .or_else(|| std::env::var("MEMORY_SEEK_LOG_FILE").ok())
        .unwrap_or_else(|| "app.log".to_string());

    // 每天创建一个新文件
    let file_appender = tracing_appender::rolling::daily(&log_dir, &log_file_name);
    // 异步写入器
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // 控制台 layer
    let stdout_layer = fmt::layer().with_ansi(true).with_writer(std::io::stdout);
    // 日志文件 layer
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
            // 全局 subscriber 已存在，丢弃文件写入器避免无用线程
            drop(guard);
            tracing::warn!("日志系统已在启动阶段初始化，跳过文件日志配置");
            None
        }
    }
}
