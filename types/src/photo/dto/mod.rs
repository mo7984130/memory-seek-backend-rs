//! photo 领域的 API 请求/响应 DTO
//!
//! 从 `domains/photo` 收敛到 types,保证 TS 导出与类型定义单一来源。
//! - 纯 DTO(无后端专属依赖)在 `lightweight` feature 下可用
//! - 依赖 `sea-orm` / token 加密的方法按需 gate 在 `orm` feature 下

pub mod collection;
pub mod comment;
#[cfg(feature = "face-engine")]
pub mod face;
pub mod person;
pub mod photo;
pub mod timeline_stat;

pub use collection::*;
pub use comment::*;
#[cfg(feature = "face-engine")]
pub use face::*;
pub use person::*;
pub use photo::*;
pub use timeline_stat::*;
