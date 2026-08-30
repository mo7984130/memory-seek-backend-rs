//! common 模块 —— 跨业务域共享的基础设施
//!
//! - [`error`]：统一错误类型
//! - [`extractors`]：axum 请求提取器
//! - [`models`]：Sea-ORM 数据库模型定义
//! - [`r`]：统一 API 响应格式
//! - [`utils`]：通用工具函数（哈希、Redis、配置等）
//! - [`macros`]：性能监控宏

#[cfg(feature = "axum")]
pub mod axum;

#[cfg(feature = "tokio")]
pub mod tokio;

pub mod error;

pub mod ext;
pub mod macros;
pub mod pipeline;
pub mod types;
pub mod utils;

pub mod time;

#[cfg(feature = "metrics")]
pub use metrics;

pub type Pool = deadpool_redis::Pool;
pub use sea_orm::ConnectionTrait as DbConn;

pub use error::AppError;
pub use error::ContextualError;
pub use error::ContextualResult;
pub use error::Result;

pub use common_macros::register_async;
