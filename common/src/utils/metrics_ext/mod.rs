/// 性能监控扩展模块
///
/// 提供基于 RAII 模式的 metrics 工具，用于自动跟踪并发度和执行耗时。
/// 仅在启用 `metrics` feature 时编译。
///
/// - `MetricsTimer`: 计时器，drop 时记录执行耗时到 histogram
/// - `MetricsConcurrencyGuard`: 并发度守卫，drop 时递减 gauge 计数
/// - `MetricsTimerExt`: Future 扩展 trait，为异步调用添加 `.timed()` 方法
mod metrics_timer;
mod metrics_concurrency_guard;
mod metrics_timer_ext;

pub use metrics_timer::MetricsTimer;
pub use metrics_concurrency_guard::MetricsConcurrencyGuard;
pub use metrics_timer_ext::MetricsTimerExt;