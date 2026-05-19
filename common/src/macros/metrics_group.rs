/// 一次性注册函数级别的性能指标：耗时计时器、并发度守卫和调用计数器
///
/// 仅在启用 `metrics` feature 时生效。注册的指标前缀为 `{crate_name}:{func}:`。
///
/// # 用法
/// ```ignore
/// metrics_group!("my_function");
/// ```
#[macro_export]
macro_rules! metrics_group {
    ($func:literal) => {
        #[cfg(feature = "metrics")]
        let _metrics_bundle = (
            $crate::utils::MetricsTimer::start(
                concat!(env!("CARGO_PKG_NAME"), ":", $func, ":duration")
            ),
            $crate::utils::MetricsConcurrencyGuard::start(
                concat!(env!("CARGO_PKG_NAME"), ":", $func, ":concurrency")
            ),
            $crate::metrics::counter!(
                concat!(env!("CARGO_PKG_NAME"), ":", $func, ":attempts")
            ).increment(1),
        );
    };
}
