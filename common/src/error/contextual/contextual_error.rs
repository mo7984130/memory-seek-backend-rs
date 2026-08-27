use std::{
    borrow::Cow,
    fmt::{Debug, Display, Formatter},
};

use tracing::Level;

use crate::error::{AppError, log_and_map};

/// 尚未记录日志的应用错误
pub struct ContextualError(Box<dyn ContextualReport>);

trait ContextualReport: Debug + Send + Sync {
    fn error(&self) -> &AppError;

    #[track_caller]
    fn emit(self: Box<Self>) -> AppError;
}

#[derive(Debug)]
struct Report<E> {
    level: Level,
    reason: &'static str,
    context: Cow<'static, str>,
    source: E,
    app_error: AppError,
}

impl<E> ContextualReport for Report<E>
where
    E: Debug + Send + Sync + 'static,
{
    fn error(&self) -> &AppError {
        &self.app_error
    }

    #[track_caller]
    fn emit(self: Box<Self>) -> AppError {
        log_and_map(
            self.level,
            self.reason,
            self.context,
            Some(&self.source),
            self.app_error,
        )
    }
}

#[derive(Debug)]
struct ReportWithoutSource {
    level: Level,
    reason: &'static str,
    context: Cow<'static, str>,
    app_error: AppError,
}

impl ContextualReport for ReportWithoutSource {
    fn error(&self) -> &AppError {
        &self.app_error
    }

    #[track_caller]
    fn emit(self: Box<Self>) -> AppError {
        log_and_map(self.level, self.reason, self.context, None, self.app_error)
    }
}

impl ContextualError {
    /// 记录错误上下文并返回对应的应用错误。
    #[track_caller]
    pub fn emit(self) -> AppError {
        self.0.emit()
    }

    /// 创建带 ERROR 日志级别和原始错误来源的上下文错误.
    pub fn error(
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        source: impl Debug + Send + Sync + 'static,
        app_error: AppError,
    ) -> Self {
        Self::with_source(Level::ERROR, reason, context, source, app_error)
    }

    /// 创建带 WARN 日志级别和原始错误来源的上下文错误.
    pub fn warn(
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        source: impl Debug + Send + Sync + 'static,
        app_error: AppError,
    ) -> Self {
        Self::with_source(Level::WARN, reason, context, source, app_error)
    }

    /// 创建不携带原始错误来源的 ERROR 上下文错误.
    pub fn error_without_source(
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> Self {
        Self::without_source(Level::ERROR, reason, context, app_error)
    }

    /// 创建不携带原始错误来源的 WARN 上下文错误.
    pub fn warn_without_source(
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> Self {
        Self::without_source(Level::WARN, reason, context, app_error)
    }

    fn with_source<E>(
        level: Level,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        source: E,
        app_error: AppError,
    ) -> Self
    where
        E: Debug + Send + Sync + 'static,
    {
        Self(Box::new(Report {
            level,
            reason,
            context: context.into(),
            source,
            app_error,
        }))
    }

    /// 构造不携带原始错误来源的延迟上下文.
    fn without_source(
        level: Level,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> Self {
        Self(Box::new(ReportWithoutSource {
            level,
            reason,
            context: context.into(),
            app_error,
        }))
    }
}

impl Debug for ContextualError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for ContextualError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.0.error(), f)
    }
}

impl std::error::Error for ContextualError {}

impl From<ContextualError> for AppError {
    #[track_caller]
    fn from(error: ContextualError) -> Self {
        error.emit()
    }
}
