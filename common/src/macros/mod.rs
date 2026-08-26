//! 通用宏
//!
//! 提供事务样板封装，以及通过 `metrics` feature 按需启用的性能监控宏。

mod caller_log;
mod current_span_name;
mod db_transaction;
mod metrics;
