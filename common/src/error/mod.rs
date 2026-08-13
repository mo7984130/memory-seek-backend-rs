/// 错误处理模块
///
/// 提供统一的应用层错误类型 `AppError`，涵盖认证、请求参数、资源不存在、权限不足等场景。
mod app_error;
pub use app_error::AppError;
mod deferred_error;
pub use deferred_error::DeferredError;
pub mod app_error_response;

/// service、controller 与请求处理边界使用的结果。
///
/// 基础设施错误不会直接实现 `Into<AppError>`；必须先进入 [`DeferredError`]，
/// 防止 `?` 绕过 service caller 边界。
pub type Result<T> = std::result::Result<T, AppError>;

/// 尚未在 service 边界转换和记录的错误结果。
pub mod deferred {
    use super::DeferredError;

    pub type Result<T> = std::result::Result<T, DeferredError>;
}
