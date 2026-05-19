/// 对代码块或表达式进行计时，记录耗时指标
///
/// 仅在启用 `metrics` feature 时生效，指标名称为 `{crate_name}:{func}:{name}:duration`。
/// 支持代码块和单表达式两种形式。
///
/// # 用法
/// ```ignore
/// timed!("my_function", "step_name", {
///     // 被计时的代码块
/// });
/// ```
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
