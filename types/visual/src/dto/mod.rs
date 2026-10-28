//! visual 领域的 API 请求/响应 DTO
//!
//! 从 `domains/visual` 收敛到 types,保证 TS 导出与类型定义单一来源。
//! - 纯 DTO(无后端专属依赖)默认可用
//! - 依赖 ORM 记录转换的方法 gate 在 `face` feature 下

pub mod collection;
pub mod comment;
pub mod face;
pub mod person;
pub mod timeline_stat;
pub mod visual;

pub use collection::*;
pub use comment::*;
pub use face::*;
pub use person::*;
pub use timeline_stat::*;
pub use visual::*;
