use crate::error::{AppError, DbErrExt};
use crate::r::R;
use std::fmt::{Debug, Display};
use tracing::{error, warn};

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

impl<E: Debug> From<ChainErr<E>> for AppError {
    fn from(e: ChainErr<E>) -> Self {
        match e {
            ChainErr::Resolved(app_err) => app_err,
            ChainErr::Pending(e) => {
                error!(error = ?e, "未处理的数据库错误，已降级为 InternalServerError");
                AppError::InternalServerError
            }
        }
    }
}

// ============================================================
// ResultExt trait
// ============================================================

/// 为 `Result<T, E>` 提供到 `AppError` 的便捷转换方法
pub trait ResultExt<T, E>: Sized {
    /// 将 `Result<T, E>` 转换为 `Result<R<T>, AppError>`，成功值包装为 `R::ok`
    fn into_ok_res(self) -> Result<R<T>, AppError>
    where
        E: Into<AppError>,
        T: serde::Serialize;

    /// 将错误映射为 `AppError::InternalServerError`，通过 `tracing::error!` 记录结构化日志
    fn trace_to_internal_err(
        self,
        reason: &'static str,
        context: &'static str,
    ) -> Result<T, AppError>
    where
        E: Debug;

    /// 将错误映射为 `AppError::BadRequest`，通过 `tracing::warn!` 记录结构化日志
    fn trace_to_bad_request_warn(
        self,
        reason: &'static str,
        context: &'static str,
    ) -> Result<T, AppError>
    where
        E: Display;

    /// 唯一键冲突 → `ChainErr::Resolved(AppError::Conflict)`，其他错误 → `ChainErr::Pending(e)`
    ///
    /// 返回 `Result<T, ChainErr<E>>` 而非 `Result<T, AppError>`，
    /// 允许后续继续链式调用 `trace_to_internal_err` 处理剩余错误。
    /// 冲突时通过 `?` 提前返回 `AppError::Conflict`（需 `From<ChainErr<E>> for AppError`）。
    ///
    /// # 示例
    /// ```rust
    /// relation.insert(db).await
    ///     .trace_conflict_warn("db_insert_conflict", "照片已存在")?
    ///     .trace_to_internal_err("db_insert_err", "添加收藏夹失败")
    /// ```
    fn trace_conflict_warn(
        self,
        reason: &'static str,
        context: &'static str,
    ) -> Result<T, ChainErr<E>>
    where
        E: Display + DbErrExt;
}

// ============================================================
// Result<T, E> 实现
// ============================================================

impl<T, E: Debug> ResultExt<T, E> for Result<T, E> {
    #[inline]
    fn into_ok_res(self) -> Result<R<T>, AppError>
    where
        E: Into<AppError>,
        T: serde::Serialize,
    {
        self.map(R::ok).map_err(Into::into)
    }

    #[inline]
    // TODO avoid double trace_to_internal_err
    fn trace_to_internal_err(
        self,
        reason: &'static str,
        context: &'static str,
    ) -> Result<T, AppError>
    where
        E: Debug,
    {
        self.map_err(|e| {
            error!(%reason, status = "error", error = ?e, "{context}");
            AppError::InternalServerError
        })
    }

    #[inline]
    fn trace_to_bad_request_warn(
        self,
        reason: &'static str,
        context: &'static str,
    ) -> Result<T, AppError>
    where
        E: Display,
    {
        self.map_err(|e| {
            warn!(%reason, status = "failed", error = %e, "{context}");
            AppError::bad_request(context)
        })
    }

    #[inline]
    fn trace_conflict_warn(
        self,
        reason: &'static str,
        context: &'static str,
    ) -> Result<T, ChainErr<E>>
    where
        E: Display + DbErrExt,
    {
        self.map_err(|e| {
            if e.is_unique_violation() {
                warn!(%reason, status = "failed", error = %e, "{context}");
                ChainErr::Resolved(AppError::Conflict(context.into()))
            } else {
                ChainErr::Pending(e)
            }
        })
    }
}

// ============================================================
// Result<T, ChainErr<E>> 实现：让链式调用继续工作
// ============================================================

impl<T, E: Debug + Display> ResultExt<T, ChainErr<E>> for Result<T, ChainErr<E>> {
    #[inline]
    fn into_ok_res(self) -> Result<R<T>, AppError>
    where
        ChainErr<E>: Into<AppError>,
        T: serde::Serialize,
    {
        self.map(R::ok).map_err(Into::into)
    }

    /// 核心：`Pending` 转 `InternalServerError`，`Resolved` 直接透传
    #[inline]
    fn trace_to_internal_err(
        self,
        reason: &'static str,
        context: &'static str,
    ) -> Result<T, AppError>
    where
        ChainErr<E>: Debug,
    {
        self.map_err(|chain| match chain {
            ChainErr::Resolved(app_err) => app_err,
            ChainErr::Pending(e) => {
                error!(%reason, status = "error", error = %e, "{context}");
                AppError::InternalServerError
            }
        })
    }

    #[inline]
    fn trace_to_bad_request_warn(
        self,
        reason: &'static str,
        context: &'static str,
    ) -> Result<T, AppError>
    where
        ChainErr<E>: Display,
    {
        self.map_err(|chain| match chain {
            ChainErr::Resolved(app_err) => app_err,
            ChainErr::Pending(e) => {
                warn!(%reason, status = "failed", error = %e, "{context}");
                AppError::bad_request(context)
            }
        })
    }

    /// 链中已有 ChainErr，冲突已处理过，直接透传
    #[inline]
    fn trace_conflict_warn(
        self,
        _reason: &'static str,
        _context: &'static str,
    ) -> Result<T, ChainErr<ChainErr<E>>>
    where
        ChainErr<E>: Display + DbErrExt,
    {
        self.map_err(ChainErr::Pending) // 透传，不重复处理
    }
}

// ============================================================
// ToOkExt trait
// ============================================================

/// 为任意类型提供便捷的 `Ok` 包装方法
pub trait ToOkExt: Sized {
    fn into_ok<E>(self) -> Result<Self, E>;
    fn ok_res(self) -> Result<Self, AppError>;
}

impl<T> ToOkExt for T {
    #[inline]
    fn into_ok<E>(self) -> Result<Self, E> {
        Ok(self)
    }

    #[inline]
    fn ok_res(self) -> Result<Self, AppError> {
        Ok(self)
    }
}
