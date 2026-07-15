use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    EnvFilter, Registry,
    fmt::{self},
    prelude::*,
};

pub fn init() -> WorkerGuard {
    // 日志输出
    // 每天创建一个新文件
    let file_appender = tracing_appender::rolling::daily("logs", "app.log");
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

    registry.init();

    guard
}
