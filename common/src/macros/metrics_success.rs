/// 递增函数调用成功计数器
///
/// 仅在启用 `metrics` feature 时生效，指标名称为 `{crate_name}:{func}:success`。
///
/// # 用法
/// ```ignore
/// metrics_success!("my_function");
/// ```
#[macro_export]
macro_rules! metrics_success {
    ($func:literal) => {
        #[cfg(feature = "metrics")]
        $crate::metrics::counter!(concat!(env!("CARGO_PKG_NAME"), ":", $func, ":success"))
            .increment(1);
    };
}
