use std::{
    borrow::Cow,
    fmt::{Debug, Display, Formatter},
};

use tracing::Level;

use crate::error::{AppError, log};

/// 尚未记录日志的应用错误
#[derive(Debug)]
pub struct ContextualError {
    level: Level,
    reason: &'static str,
    context: Cow<'static, str>,
    source: Option<Box<dyn Debug + Send + Sync>>,
    app_error: AppError,
}

impl ContextualError {
    /// 记录错误上下文并返回对应的应用错误。
    #[track_caller]
    pub fn emit(self) -> AppError {
        let Self {
            level,
            reason,
            context,
            source,
            app_error,
        } = self;

        log(level, reason, context, source.as_deref(), app_error)
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
        Self {
            level,
            reason,
            context: context.into(),
            source: Some(Box::new(source)),
            app_error,
        }
    }

    /// 构造不携带原始错误来源的延迟上下文.
    fn without_source(
        level: Level,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> Self {
        Self {
            level,
            reason,
            context: context.into(),
            source: None,
            app_error,
        }
    }
}

impl Display for ContextualError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.app_error, f)
    }
}

impl std::error::Error for ContextualError {}

impl From<ContextualError> for AppError {
    #[track_caller]
    fn from(error: ContextualError) -> Self {
        error.emit()
    }
}
