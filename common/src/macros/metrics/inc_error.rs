/// 递增错误分类计数器
///
/// 指标名称为 `{crate}:{func}:errors:{kind}`，`kind` 取错误分类白名单
/// （`db` / `redis` / `s3` / `smtp` / `validation` / `auth` / `not_found` /
/// `conflict` / `internal` 或领域专属如 `download` / `decode` / `detect` / `insert`）。
///
/// 仅在启用 `metrics` feature 时生效。
///
/// # 用法
/// ```ignore
/// // 自动使用当前 span 名
/// inc_error!("db");
/// // 显式指定函数名
/// inc_error!("face:compute", "detect");
/// ```
#[macro_export]
macro_rules! inc_error {
    ($kind:literal) => {{
        #[cfg(feature = "metrics")]
        $crate::metrics::counter!(format!(
            "{}:{}:errors:{}",
            env!("CARGO_PKG_NAME"),
            $crate::current_span_name!(),
            $kind
        ))
        .increment(1);
        #[cfg(not(feature = "metrics"))]
        {}
    }};
    ($func_name:literal, $kind:literal) => {{
        #[cfg(feature = "metrics")]
        $crate::metrics::counter!(concat!(
            env!("CARGO_PKG_NAME"),
            ":",
            $func_name,
            ":errors:",
            $kind
        ))
        .increment(1);
        #[cfg(not(feature = "metrics"))]
        {}
    }};
}
