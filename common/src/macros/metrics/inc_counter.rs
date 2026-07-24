/// 递增 counter 指标（累积计数）
///
/// 指标名通过 `metrics_name!` 自动生成，前缀为 `{crate}:{func}:`。
/// 仅在启用 `metrics` feature 时生效。
///
/// # 用法
/// ```ignore
/// // 自动使用当前 span 名
/// inc_counter!("processed", 1);
/// // 显式指定函数名
/// inc_counter!("face:compute", "photos_processed", count as u64);
/// ```
#[macro_export]
macro_rules! inc_counter {
    ($step:literal, $value:expr) => {
        #[cfg(feature = "metrics")]
        $crate::metrics::counter!($crate::metrics_name!($step)).increment($value);
    };
    ($func_name:literal, $step:literal, $value:expr) => {
        #[cfg(feature = "metrics")]
        $crate::metrics::counter!($crate::metrics_name!($func_name, $step)).increment($value);
    };
}
