//! 性能监控宏
//!
//! 提供函数级别的耗时计时、并发度跟踪和调用计数宏，通过 `metrics` feature 按需启用。

mod metrics_group;
mod metrics_success;
mod metrics_timed;
mod metrics_timer_name;
