/// 设置 gauge 指标的绝对值
///
/// 指标名通过 `metrics_name!` 自动生成，前缀为 `{crate}:{func}:`。
/// 仅在启用 `metrics` feature 时生效。
///
/// # 用法
/// ```ignore
/// // 自动使用当前 span 名
/// set_gauge!("batch", 5.0);
/// // 显式指定函数名
/// set_gauge!("face:compute", "mode", 1.0);
/// ```
#[macro_export]
macro_rules! set_gauge {
    ($step:literal, $value:expr) => {
        #[cfg(feature = "metrics")]
        $crate::metrics::gauge!($crate::metrics_name!($step)).set($value);
    };
    ($func_name:literal, $step:literal, $value:expr) => {
        #[cfg(feature = "metrics")]
        $crate::metrics::gauge!($crate::metrics_name!($func_name, $step)).set($value);
    };
}
