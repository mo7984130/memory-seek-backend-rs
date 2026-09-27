//! 关系型持久化能力:连接工具、表元数据、事务步骤管道与事务宏。

mod db_transaction;
pub mod pipeline;
pub mod utils;

/// 事务宏以 crate 根上的 `error` 路径引用统一错误,故在此重导出 `common-core`
/// 的错误模块,使宏在展开处能解析。
pub use common_core::error;
