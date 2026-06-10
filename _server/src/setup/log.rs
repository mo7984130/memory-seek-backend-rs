use tracing_subscriber::{EnvFilter, Registry, fmt, prelude::*};

/// 初始化日志系统
///
/// 配置 tracing 日志输出到控制台（带 ANSI 颜色）和日志文件（JSON 格式，
/// 按天滚动到 `logs/app.log`）。日志级别默认为 `info`，`sqlx` 为 `warn`。
/// 启用 `metrics` feature 时会额外添加 MetricsLayer 以关联 tracing 和 metrics。
pub fn init_log() {
    // 日志输出
    // 每天创建一个新文件
    let file_appender = tracing_appender::rolling::daily("logs", "app.log");
    // 异步写入器
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // 控制台layer
    let stdout_layer = fmt::layer()
        .with_ansi(true)
        .with_writer(std::io::stdout);
    // 日志文件layer
    let file_layer = fmt::layer()
        .json()
        .with_writer(non_blocking);

    let registry = Registry::default()
        .with(EnvFilter::new("info,sqlx=warn"))
        .with(stdout_layer)
        .with(file_layer);

    #[cfg(feature = "metrics")]
    let registry = {
        use metrics_tracing_context::MetricsLayer;
        registry.with(MetricsLayer::new())
    };

    registry.init();
}
