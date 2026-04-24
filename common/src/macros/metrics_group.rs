#[macro_export]
macro_rules! metrics_group {
    ($name:literal) => {
        #[cfg(feature = "metrics")]
        let _metrics_bundle = (
            $crate::utils::MetricsTimer::start(concat!($name, ":duration")),
            $crate::utils::MetricsConcurrencyGuard::start(concat!($name, ":concurrency")),
            $crate::metrics::counter!(concat!($name, ":attempts")).increment(1),
        );
    };
}
