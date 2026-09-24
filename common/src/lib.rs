//! common 模块 —— 跨业务域共享的基础设施门面
//!
//! 能力 crate(按职责拆分,依赖方向单向指向 `common-core`):
//! - `common-core`：统一错误、时间、扩展 trait、通用值类型
//! - `common-crypto`：口令哈希与令牌加解密
//!
//! 本 crate 仍承载尚未拆出的能力(axum 适配、持久化工具、缓存、可观测性、
//! 运行时工具),并**重导出**已下沉 / 已拆出的部分,保持既有 `common::…` 路径不变。

#[cfg(feature = "axum")]
pub mod axum;

#[cfg(feature = "tokio")]
pub mod tokio;

pub mod ext;
pub mod macros;
pub mod pipeline;
pub mod utils;

/// 统一错误(已下沉 `common-core`)。
pub use common_core::error;
/// 时间类型与辅助(已下沉 `common-core`)。
pub use common_core::time;
/// 通用值类型:变更包装与游标分页(已下沉 `common-core`)。
pub use common_core::types;

pub use common_core::{AppError, ContextualError, ContextualResult, Result};

#[cfg(feature = "metrics")]
pub use metrics;

pub type Pool = deadpool_redis::Pool;
pub use sea_orm::ConnectionTrait as DbConn;

pub use common_macros::async_boxed;
pub use common_macros::register_async;
