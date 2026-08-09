use tracing::info;
use tracing_subscriber::filter::{EnvFilter, filter_fn};
use tracing_subscriber::{Registry, fmt, prelude::*};

/// 初始化日志系统（stdout/stderr 分流输出），在 main 入口处尽早调用。
///
/// 不自行维护日志文件，日志统一输出到 stdout / stderr，
/// 由 systemd 的 journald 捕获管理（见仓库根目录 memory-seek-server.service）。
///
/// 注意: tracing 的 Level 排序中 ERROR 为最低级别（Error < Warn < Info < Debug < Trace），
/// 因此 stdout 用 `level > ERROR` 承接 warn 及以下，stderr 用 `level <= ERROR` 承接 error。
/// - stdout: trace/debug/info/warn
/// - stderr: error 及以上
/// - 日志级别由环境变量 `RUST_LOG` 控制（默认 info）
pub fn init() {
    // stdout layer: 承接 warn 及以下级别（排除 error）
    let stdout_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(std::io::stdout)
        .with_filter(filter_fn(|metadata| {
            metadata.level() > &tracing::Level::ERROR
        }));

    // stderr layer: 承接 error 及以上级别
    let stderr_layer = fmt::layer()
        .with_ansi(false)
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
