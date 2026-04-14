#[macro_export]
macro_rules! metrics_success {
    ($name:literal) => {
        #[cfg(feature = "metrics")]
        metrics::counter!(concat!($name, "_success")).increment(1);
    };
}
