//! HTTP 适配能力:axum 提取器、统一路由与响应辅助。
//!
//! 统一响应格式 `R` 与 `AppError` 的响应实现位于 `common-core`(孤儿规则要求
//! 与错误类型同 crate),此处重导出以便提取器与调用方共用。

pub mod body_util;
pub mod controller_router;
pub mod ext;
pub mod extractors;

pub use common_core::r::{ErrR, R, SucR};
