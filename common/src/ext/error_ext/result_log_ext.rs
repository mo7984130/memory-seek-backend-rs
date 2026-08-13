use std::fmt::Debug;

use crate::{
    error::AppError,
    ext::error_ext::base::{log_err_with_source, log_warn_with_source},
};

/// 为失败结果记录带来源的日志，并映射为应用错误。
///
/// 适用于 controller、extractor 等错误边界；与仅构造
/// [`ContextualError`](crate::error::ContextualError) 的上下文化扩展不同，
/// 此 trait 会立即记录日志。
pub trait ResultLogExt<T> {
    #[track_caller]
    fn log_err(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> std::result::Result<T, AppError>;

    #[track_caller]
    fn log_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> std::result::Result<T, AppError>;
}

impl<T, E> ResultLogExt<T> for std::result::Result<T, E>
where
    E: Debug,
{
    #[track_caller]
    fn log_err(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> std::result::Result<T, AppError> {
        self.map_err(|source| log_err_with_source(reason, context, source, app_error))
    }

    #[track_caller]
    fn log_warn(
        self,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> std::result::Result<T, AppError> {
        self.map_err(|source| log_warn_with_source(reason, context, source, app_error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_warn_maps_error_to_app_error() {
        let result = Err::<(), _>("invalid").log_warn(
            "invalid_input",
            "输入无效",
            AppError::bad_request("输入无效"),
        );

        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }

    #[test]
    fn log_err_preserves_success_value() {
        let result =
            Ok::<_, ()>(42).log_err("unexpected", "不应失败", AppError::InternalServerError);

        assert_eq!(result.unwrap(), 42);
    }
}
