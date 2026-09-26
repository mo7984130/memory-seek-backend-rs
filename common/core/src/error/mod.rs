/// 错误处理模块
///
/// 提供统一的应用层错误类型 `AppError`，涵盖认证、请求参数、资源不存在、权限不足等场景。
mod app_error;
pub use app_error::AppError;
pub type Result<T> = std::result::Result<T, AppError>;

/// axum 响应适配(受孤儿规则约束, 必须与本类型同 crate)。
#[cfg(feature = "axum")]
mod axum_impl;

pub mod contextual;
pub use contextual::ContextualError;
pub use contextual::ContextualResult;

mod base;
pub(crate) use base::log_and_map;
