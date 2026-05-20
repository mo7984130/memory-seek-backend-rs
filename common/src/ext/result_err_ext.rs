use crate::{
    error::{AppError, DbErrExt},
    ext::base::{log_bad_request_warn, log_conflict_warn, log_internal_err},
};
use std::fmt::Debug;

// TODO ChainErr

// ============================================================
// ChainErr：链式错误处理的中间态
// ============================================================

/// 链式错误处理的中间态
///
/// 用于 `trace_conflict_warn` 等「部分处理」方法：
/// - 已识别的错误（如冲突）转为 `Resolved(AppError)`，后续链直接透传
/// - 未识别的错误保留为 `Pending(E)`，等待下一个链节处理
pub enum ChainErr<E> {
    /// 已转换好的 AppError
    Resolved(AppError),
    /// 尚未处理的原始错误
    Pending(E),
}

// impl<E: Debug> From<ChainErr<E>> for AppError {
//     fn from(e: ChainErr<E>) -> Self {
//         match e {
//             ChainErr::Resolved(app_err) => app_err,
//             ChainErr::Pending(e) => {
//                 error!(error = ?e, "未处理的数据库错误，已降级为 InternalServerError");
//                 AppError::InternalServerError
//             }
//         }
//     }
// }
//

// ============================================================
// ResultExt trait
// ============================================================

/// 为 `Result<T, E>` 提供到 `AppError` 的便捷转换方法
pub trait ResultErrExt<T, E: Debug> {
    /// 将错误映射为 `AppError::InternalServerError`，通过 `tracing::error!` 记录结构化日志
    fn trace_to_internal_err(
        self,
        reason: &'static str,
        context: &'static str,
    ) -> Result<T, AppError>;

    /// 将错误映射为 `AppError::BadRequest`，通过 `tracing::warn!` 记录结构化日志
    fn trace_to_bad_request_warn(
        self,
        reason: &'static str,
        context: &'static str,
    ) -> Result<T, AppError>;

    fn trace_conflict_warn(
        self,
        reason: &'static str,
        context: &'static str,
    ) -> Result<T, ChainErr<E>>
    where
        E: DbErrExt;
}

// ============================================================
// Result<T, E> 实现
// ============================================================

impl<T, E: Debug> ResultErrExt<T, E> for Result<T, E> {
    #[inline]
    fn trace_to_internal_err(
        self,
        reason: &'static str,
        context: &'static str,
    ) -> Result<T, AppError> {
        self.map_err(|e| log_internal_err(reason, context, e))
    }

    #[inline]
    fn trace_to_bad_request_warn(
        self,
        reason: &'static str,
        msg: &'static str,
    ) -> Result<T, AppError> {
        self.map_err(|e| log_bad_request_warn(reason, msg, e))
    }

    #[inline]
    fn trace_conflict_warn(self, reason: &'static str, msg: &'static str) -> Result<T, ChainErr<E>>
    where
        E: DbErrExt,
    {
        self.map_err(|e| {
            if e.is_unique_violation() {
                ChainErr::Resolved(log_conflict_warn(reason, msg, e))
            } else {
                ChainErr::Pending(e)
            }
        })
    }
}

pub fn make_internal_err(reason: &'static str, context: &'static str) -> AppError {
    log_internal_err(reason, context, ())
}

pub fn make_bad_request_err(reason: &'static str, msg: &'static str) -> AppError {
    log_bad_request_warn(reason, msg, ())
}
