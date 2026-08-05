/// 错误处理模块
///
/// 提供统一的应用层错误类型 `AppError`，涵盖认证、请求参数、资源不存在、权限不足等场景。
mod app_error;
pub use app_error::AppError;
pub mod app_error_response;
pub mod db_error;
pub mod mutex_error;
pub mod redis_error;
pub mod serde_error;
#[cfg(feature = "tokio")]
pub mod tokio_error;

pub type Result<T> = std::result::Result<T, AppError>;
