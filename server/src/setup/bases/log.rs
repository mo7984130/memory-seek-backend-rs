use std::io::IsTerminal;

use tracing::info;
use tracing_subscriber::filter::{EnvFilter, filter_fn};
use tracing_subscriber::{Registry, fmt, prelude::*};

/// 初始化日志系统，按级别分流 stdout/stderr 并由 `RUST_LOG` 控制级别；须在入口尽早调用
pub fn init() {
    // stdout layer: 承接 warn 及以下级别（排除 error）
    let stdout_layer = fmt::layer()
        .with_ansi(std::io::stdout().is_terminal())
        .with_writer(std::io::stdout)
        .with_filter(filter_fn(|metadata| {
            metadata.level() > &tracing::Level::ERROR
        }));

    // stderr layer: 承接 error 及以上级别
    let stderr_layer = fmt::layer()
        .with_ansi(std::io::stderr().is_terminal())
        .with_writer(std::io::stderr)
        .with_filter(filter_fn(|metadata| {
            metadata.level() <= &tracing::Level::ERROR
        }));

    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let registry = Registry::default()
        .with(EnvFilter::new(format!("{},sqlx=warn", log_level)))
        .with(stdout_layer)
        .with(stderr_layer);

    #[cfg(feature = "metrics")]
    let registry = {
        use metrics_tracing_context::MetricsLayer;
        registry.with(MetricsLayer::new())
    };

    registry.init();
    info!("日志系统初始化完成");
}
