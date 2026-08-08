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
    () => {
        #[cfg(feature = "metrics")]
        let _metrics_guard = {
            $crate::metrics::counter!($crate::metrics_name!("attempts")).increment(1);

            $crate::metrics_timer!()
        };
    };

    ($func_name:literal) => {
        #[cfg(feature = "metrics")]
        let _metrics_guard = {
            $crate::metrics::counter!($crate::metrics_name!($func_name, "attempts")).increment(1);

            $crate::metrics_timer!($func_name, "")
        };
    };
}
