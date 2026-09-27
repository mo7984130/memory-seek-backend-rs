//! 可观测性能力:metrics 宏与计时/并发工具。
//!
//! 各宏以本 crate 根上的路径互相引用(`metrics` crate 与 `metrics_name!` 等),
//! 因此 `metrics` 在本 crate 根上重导出。

pub use metrics;

mod gauges;
pub use gauges::*;

mod timer;
pub use timer::MetricsTimer;

mod timer_ext;
pub use timer_ext::MetricsTimerExt;

mod current_span_name;
mod macros;
