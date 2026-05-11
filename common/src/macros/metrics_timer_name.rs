#[macro_export]
macro_rules! metrics_timer_name {
    ($func:literal, $name:literal) => {
        concat!(env!("CARGO_PKG_NAME"), ":", $func, ":", $name, ":duration")
    };
}
