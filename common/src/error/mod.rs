/// 错误处理模块
///
/// 提供统一的应用层错误类型 `AppError`，涵盖认证、请求参数、资源不存在、权限不足等场景。
/// `AppError` 自动实现 axum 的 `IntoResponse`，可直接作为 handler 返回值。
mod app_error;
pub use app_error::AppError;
