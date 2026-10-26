use std::{
    borrow::Cow,
    fmt::{Debug, Display, Formatter},
};

use tracing::Level;

use crate::error::{AppError, log};

/// 尚未记录日志的上下文错误
///
/// 泛型参数 `E` 是“最终错误载荷”：`emit` 记录日志后原样返回它，因此本类型不绑定到
/// 任何具体错误类型。默认载荷为 [`AppError`]，以保持既有调用点无需标注类型参数。
#[derive(Debug)]
pub struct ContextualError<E = AppError> {
    level: Level,
    reason: &'static str,
    context: Cow<'static, str>,
    source: Option<Box<dyn Debug + Send + Sync>>,
    payload: E,
}

impl<E> ContextualError<E> {
    /// 记录错误日志并返回携带的载荷
    #[track_caller]
    pub fn emit(self) -> E {
        let Self {
            level,
            reason,
            context,
            source,
            payload,
        } = self;

        log(level, reason, context, source.as_deref(), payload)
    }

    /// 创建带 ERROR 级别与原始错误来源的上下文错误
    pub fn error(
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        source: impl Debug + Send + Sync + 'static,
        payload: E,
    ) -> Self {
        Self::with_source(Level::ERROR, reason, context, source, payload)
    }

    /// 创建带 WARN 级别与原始错误来源的上下文错误
    pub fn warn(
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        source: impl Debug + Send + Sync + 'static,
        payload: E,
    ) -> Self {
        Self::with_source(Level::WARN, reason, context, source, payload)
    }

    /// 创建不携带原始错误来源的 ERROR 级别上下文错误
    pub fn error_without_source(
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> Self {
        Self::without_source(Level::ERROR, reason, context, payload)
    }

    /// 创建不携带原始错误来源的 WARN 级别上下文错误
    pub fn warn_without_source(
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> Self {
        Self::without_source(Level::WARN, reason, context, payload)
    }

    /// 构造携带原始错误来源的上下文错误
    fn with_source<S>(
        level: Level,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        source: S,
        payload: E,
    ) -> Self
    where
        S: Debug + Send + Sync + 'static,
    {
        Self {
            level,
            reason,
            context: context.into(),
            source: Some(Box::new(source)),
            payload,
        }
    }

    /// 构造不携带原始错误来源的上下文错误
    fn without_source(
        level: Level,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> Self {
        Self {
            level,
            reason,
            context: context.into(),
            source: None,
            payload,
        }
    }
}

impl<E: Display> Display for ContextualError<E> {
    /// 委托底层载荷的显示实现
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.payload, f)
    }
}

impl<E: Debug + Display> std::error::Error for ContextualError<E> {}

impl From<ContextualError<AppError>> for AppError {
    /// 转换为 `AppError`，并记录错误日志
    #[track_caller]
    fn from(error: ContextualError<AppError>) -> Self {
        error.emit()
    }
}
