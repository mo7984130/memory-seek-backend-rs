/// 性能监控扩展模块
///
/// 提供基于 RAII 模式的 metrics 工具，用于自动跟踪并发度和执行耗时。
///
/// - `MetricsTimer`: 计时器，drop 时记录执行耗时到 histogram
/// - `MetricsTimerExt`: Future 扩展 trait，为异步调用添加 `.timed()` 方法
/// - `GaugeGuard`: gauge 守卫，创建时 +1 销毁时 -1
mod gauges;
mod timer;
mod timer_ext;

pub use gauges::*;
pub use timer::MetricsTimer;
pub use timer_ext::MetricsTimerExt;
