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
