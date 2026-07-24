/// 创建以 `{crate}:{func}[:{step}]:duration_seconds` 命名的计时器
///
/// 仅在启用 `metrics` feature 时生效，无参时自动通过 `current_span_name!` 获取函数名。
#[macro_export]
macro_rules! metrics_timer {
    () => {
        $crate::utils::MetricsTimer::start($crate::metrics_name!("duration_seconds"))
    };

    ($step:literal) => {
        $crate::utils::MetricsTimer::start(format!(
            "{}:{}:{}:duration_seconds",
            env!("CARGO_PKG_NAME"),
            $crate::current_span_name!(),
            $step
        ))
    };

    ($func_name:literal, $step:literal) => {
        $crate::utils::MetricsTimer::start(concat!(
            $crate::metrics_name!($func_name, $step),
            ":duration_seconds"
        ))
    };
}

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
    ($step:expr, $block:block) => {{
        #[cfg(feature = "metrics")]
        let _metrics_timer = $crate::metrics_timer!($step);
        $block
    }};
    ($step:expr, $entry:expr) => {{
        #[cfg(feature = "metrics")]
        let _metrics_timer = $crate::metrics_timer!($step);
        $entry
    }};

    ($func_name:literal, $step:expr, $block:block) => {{
        #[cfg(feature = "metrics")]
        let _metrics_timer = $crate::metrics_timer!($func_name, $step);
        $block
    }};
    ($func_name:literal, $step:expr, $entry:expr) => {{
        #[cfg(feature = "metrics")]
        let _metrics_timer = $crate::metrics_timer!($func_name, $step);
        $entry
    }};
}
