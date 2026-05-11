#[macro_export]
macro_rules! timed {
    ($func:literal, $name:expr, $block:block) => {{
        #[cfg(feature = "metrics")]
        let _t = $crate::utils::MetricsTimer::start(
            concat!(env!("CARGO_PKG_NAME"), ":", $func, ":", $name, ":duration")
        );
        $block
    }};
    ($func:literal, $name:expr, $entry:expr) => {{
        #[cfg(feature = "metrics")]
        let _t = $crate::utils::MetricsTimer::start(
            concat!(env!("CARGO_PKG_NAME"), ":", $func, ":", $name, ":duration")
        );
        $entry
    }};
}
