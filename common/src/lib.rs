//! common 模块 —— 跨业务域共享的基础设施
//!
//! - [`error`]：统一错误类型
//! - [`extractors`]：axum 请求提取器
//! - [`models`]：Sea-ORM 数据库模型定义
//! - [`r`]：统一 API 响应格式
//! - [`utils`]：通用工具函数（哈希、Redis、配置等）
//! - [`macros`]：性能监控宏

pub mod error;
pub use error::Result;
pub mod extractors;
pub mod models;
pub mod r;
pub mod traits;
pub mod utils;

pub mod macros;

pub mod ext;

#[cfg(feature = "metrics")]
pub use metrics;
