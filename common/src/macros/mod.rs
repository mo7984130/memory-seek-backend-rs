//! 通用宏门面:`db_transaction!` 在 `common-db`,性能监控宏在 `common-metrics`。

pub use common_db::db_transaction;

#[cfg(feature = "metrics")]
pub use common_metrics::*;
