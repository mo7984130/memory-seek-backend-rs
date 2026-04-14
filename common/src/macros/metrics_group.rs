#[macro_export]
macro_rules! metrics_group {
    ($name:literal) => {
        #[cfg(feature = "metrics")]
        let _metrics_bundle = (
            $crate::utils::MetricsTimer::start(concat!($name, "_total_seconds")),
            $crate::utils::MetricsConcurrencyGuard::start(concat!($name, "_concurrency")),
            // 注意：这里需要确保 metrics crate 已经在当前作用域或使用全路径
            metrics::counter!(concat!($name, "_attempts")).increment(1),
        );
    };
}
