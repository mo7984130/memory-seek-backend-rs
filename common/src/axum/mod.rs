pub mod body_util;
pub mod controller_router;
pub mod ext;
pub mod extractors;

/// 统一 API 响应格式已下沉 `common-core`(与 `AppError` 的 `IntoResponse` 实现同处,
/// 以满足孤儿规则),此处重导出以保持 `common::axum::{R, SucR, ErrR}` 路径不变。
pub use common_core::r::{ErrR, R, SucR};
