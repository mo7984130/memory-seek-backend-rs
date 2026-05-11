#[macro_export]
macro_rules! metrics_success {
    ($func:literal) => {
        #[cfg(feature = "metrics")]
        $crate::metrics::counter!(
            concat!(env!("CARGO_PKG_NAME"), ":", $func, ":success")
        ).increment(1);
    };
}
