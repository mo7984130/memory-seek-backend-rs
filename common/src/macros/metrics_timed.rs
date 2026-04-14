#[macro_export]
macro_rules! timed {
    ($metric:expr, $block:block) => {{
        #[cfg(feature = "metrics")]
        let _t = $crate::utils::MetricsTimer::start($metric);
        $block
    }};
    // 这种模式可以支持 timed!("name", expr) 这种简写
    ($metric:expr, $entry:expr) => {{
        #[cfg(feature = "metrics")]
        let _t = $crate::utils::MetricsTimer::start($metric);
        $entry
    }};
}
