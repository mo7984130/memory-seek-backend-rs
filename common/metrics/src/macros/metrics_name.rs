#[macro_export]
macro_rules! metrics_name {
    ($step:literal) => {
        format!(
            "{}:{}:{}",
            env!("CARGO_PKG_NAME"),
            $crate::current_span_name!(),
            $step
        )
    };
    ($func_name:literal, $step:literal) => {
        concat!(env!("CARGO_PKG_NAME"), ":", $func_name, ":", $step)
    };
}
