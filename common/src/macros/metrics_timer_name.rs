/// 拼接计时器指标名称，格式为 `{crate_name}:{func}:{name}:duration`
///
/// # 用法
/// ```ignore
/// let metric_name = metrics_timer_name!("my_function", "step");
/// ```
#[macro_export]
macro_rules! metrics_timer_name {
    ($func:literal, $name:literal) => {
        concat!(env!("CARGO_PKG_NAME"), ":", $func, ":", $name, ":duration")
    };
}
