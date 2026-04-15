#[macro_export]
macro_rules! metrics_success {
    ($name:literal) => {
        #[cfg(feature = "metrics")]
        $crate::metrics::counter!(concat!($name, ":success")).increment(1);
    };
}
