//! common 模块 —— 跨业务域共享的基础设施门面
//!
//! 能力 crate(按职责拆分,依赖方向单向指向 `common-core`):
//! - `common-core`：统一错误、时间、扩展 trait、通用值类型、响应格式
//! - `common-crypto`：口令哈希与令牌加解密
//! - `common-db`：持久化工具、事务管道与事务宏
//! - `common-cache`：Redis 连接池与扩展
//! - `common-metrics`：metrics 宏与计时工具(metrics feature)
//! - `common-web`：axum 提取器与路由辅助(axum feature)
//! - `common-runtime`：任务管理器与事件(tokio feature)
//!
//! 本 crate 只做重导出,保持既有 `common::…` 路径不变。

#[cfg(feature = "axum")]
pub mod axum;

#[cfg(feature = "tokio")]
pub mod tokio;

pub mod ext;
pub mod macros;
pub mod utils;

/// 事务步骤管道(`common-db`)。
pub use common_db::pipeline;

/// 统一错误(`common-core`)。
pub use common_core::error;
/// 时间类型与辅助(`common-core`)。
pub use common_core::time;
/// 通用值类型:变更包装与游标分页(`common-core`)。
pub use common_core::types;
pub use common_core::{AppError, ContextualError, ContextualResult, DbConn, Result};

/// 事务宏(`common-db`)。
pub use common_db::db_transaction;

/// 性能监控宏与工具(`common-metrics`,由 `metrics` feature 启用)。
#[cfg(feature = "metrics")]
pub use common_metrics::*;

/// Redis 连接池(`common-cache`)。
pub use common_cache::Pool;

pub use common_macros::async_boxed;
pub use common_macros::register_async;
