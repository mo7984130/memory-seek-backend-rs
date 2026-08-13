use std::fmt::{Debug, Display, Formatter};
use std::panic::Location;

use tracing::Level;

use crate::{error::AppError, ext::error_ext::base::log_and_map_at};

/// 尚未记录日志的应用错误。
///
/// mapper、repo 和基础设施层使用该错误保留底层原因；错误进入 service 后，
/// 由 `From<ContextualError> for AppError` 在 service 的 `?` 调用点统一记录。
pub struct ContextualError(Box<dyn ContextualReport>);

trait ContextualReport: Debug + Send + Sync {
    fn app_error(&self) -> &AppError;

    #[track_caller]
    fn emit(self: Box<Self>) -> AppError;
}

#[derive(Debug)]
struct Report<E> {
    level: Level,
    reason: &'static str,
    context: &'static str,
    source: E,
    app_error: AppError,
}

impl<E> ContextualReport for Report<E>
where
    E: Debug + Send + Sync + 'static,
{
    fn app_error(&self) -> &AppError {
        &self.app_error
    }

    #[track_caller]
    fn emit(self: Box<Self>) -> AppError {
        log_and_map_at(
            self.level,
            self.reason,
            self.context,
            Some(&self.source),
            self.app_error,
            Location::caller(),
        )
    }
}

#[derive(Debug)]
struct ReportWithoutSource {
    level: Level,
    reason: &'static str,
    context: &'static str,
    app_error: AppError,
}

impl ContextualReport for ReportWithoutSource {
    fn app_error(&self) -> &AppError {
        &self.app_error
    }

    #[track_caller]
    fn emit(self: Box<Self>) -> AppError {
        log_and_map_at(
            self.level,
            self.reason,
            self.context,
            None,
            self.app_error,
            Location::caller(),
        )
    }
}

impl ContextualError {
    /// 记录错误上下文并返回对应的应用错误。
    ///
    /// 该方法会消费错误，确保同一份上下文最多被记录一次。适用于补偿操作失败时
    /// 需要记录错误、但不应覆盖原始错误的场景。
    #[track_caller]
    pub fn emit(self) -> AppError {
        self.0.emit()
    }

    pub fn error(
        reason: &'static str,
        context: &'static str,
        source: impl Debug + Send + Sync + 'static,
        app_error: AppError,
    ) -> Self {
        Self::with_source(Level::ERROR, reason, context, source, app_error)
    }

    pub fn warn(
        reason: &'static str,
        context: &'static str,
        source: impl Debug + Send + Sync + 'static,
        app_error: AppError,
    ) -> Self {
        Self::with_source(Level::WARN, reason, context, source, app_error)
    }

    pub fn error_without_source(
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Self {
        Self::without_source(Level::ERROR, reason, context, app_error)
    }

    pub fn warn_without_source(
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Self {
        Self::without_source(Level::WARN, reason, context, app_error)
    }

    fn with_source<E>(
        level: Level,
        reason: &'static str,
        context: &'static str,
        source: E,
        app_error: AppError,
    ) -> Self
    where
        E: Debug + Send + Sync + 'static,
    {
        Self(Box::new(Report {
            level,
            reason,
            context,
            source,
            app_error,
        }))
    }

    fn without_source(
        level: Level,
        reason: &'static str,
        context: &'static str,
        app_error: AppError,
    ) -> Self {
        Self(Box::new(ReportWithoutSource {
            level,
            reason,
            context,
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
        Display::fmt(self.0.app_error(), f)
    }
}

impl std::error::Error for ContextualError {}

impl From<std::io::Error> for ContextualError {
    fn from(error: std::io::Error) -> Self {
        Self::error(
            "io_error",
            "I/O 操作失败",
            error,
            AppError::InternalServerError,
        )
    }
}

#[cfg(feature = "contextual-sea-orm")]
impl From<sea_orm::DbErr> for ContextualError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::error("db_err", "数据库错误", error, AppError::InternalServerError)
    }
}

#[cfg(feature = "contextual-redis")]
impl From<deadpool_redis::PoolError> for ContextualError {
    fn from(error: deadpool_redis::PoolError) -> Self {
        Self::error(
            "redis_err",
            "Redis错误",
            error,
            AppError::InternalServerError,
        )
    }
}

#[cfg(feature = "contextual-redis")]
impl From<redis::RedisError> for ContextualError {
    fn from(error: redis::RedisError) -> Self {
        Self::error(
            "redis_err",
            "Redis错误",
            error,
            AppError::InternalServerError,
        )
    }
}

#[cfg(feature = "contextual-cache")]
impl From<multi_level_cache::CacheError> for ContextualError {
    fn from(error: multi_level_cache::CacheError) -> Self {
        Self::warn(
            "cache_err",
            "缓存错误",
            error,
            AppError::InternalServerError,
        )
    }
}

#[cfg(feature = "contextual-serde")]
impl From<serde_json::Error> for ContextualError {
    fn from(error: serde_json::Error) -> Self {
        Self::warn(
            "serde_json_error",
            "serde_json错误",
            error,
            AppError::InternalServerError,
        )
    }
}

#[cfg(feature = "contextual-tokio")]
impl From<tokio::sync::AcquireError> for ContextualError {
    fn from(error: tokio::sync::AcquireError) -> Self {
        Self::error(
            "tokio_semaphore_error",
            "信号量错误",
            error,
            AppError::InternalServerError,
        )
    }
}

#[cfg(feature = "contextual-tokio")]
impl From<tokio::task::JoinError> for ContextualError {
    fn from(error: tokio::task::JoinError) -> Self {
        Self::error(
            "tokio_join_error",
            "Tokio 任务执行失败",
            error,
            AppError::InternalServerError,
        )
    }
}

impl From<ContextualError> for AppError {
    #[track_caller]
    fn from(error: ContextualError) -> Self {
        error.emit()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::fmt::MakeWriter;

    use super::*;

    #[derive(Clone, Default)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    struct BufferWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for BufferWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for LogBuffer {
        type Writer = BufferWriter;

        fn make_writer(&'a self) -> Self::Writer {
            BufferWriter(Arc::clone(&self.0))
        }
    }

    fn service_boundary(error: ContextualError) -> (u32, crate::Result<()>) {
        let expected_line = line!() + 2;
        let result = (|| -> crate::Result<()> {
            Err::<(), ContextualError>(error)?;
            Ok(())
        })();
        (expected_line, result)
    }

    #[test]
    fn conversion_logs_the_service_boundary_location() {
        let buffer = LogBuffer::default();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(buffer.clone())
            .finish();

        let (expected_line, app_error) = tracing::subscriber::with_default(subscriber, || {
            service_boundary(ContextualError::error(
                "db_err",
                "数据库错误",
                "connection refused",
                AppError::InternalServerError,
            ))
        });

        let app_error = app_error.unwrap_err();
        assert!(matches!(app_error, AppError::InternalServerError));
        let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        assert!(
            output.contains("common/src/error/contextual_error.rs"),
            "unexpected log output: {output}"
        );
        assert!(
            output.contains(&format!(
                "common/src/error/contextual_error.rs:{expected_line}"
            )),
            "unexpected log output: {output}"
        );
        assert_eq!(output.matches("reason=\"db_err\"").count(), 1);
    }

    #[test]
    fn contextual_error_is_pointer_sized() {
        assert_eq!(
            std::mem::size_of::<ContextualError>(),
            2 * std::mem::size_of::<usize>()
        );
    }

    fn assert_from<T>()
    where
        ContextualError: From<T>,
    {
    }

    #[test]
    fn io_conversion_is_available() {
        assert_from::<std::io::Error>();
    }

    #[test]
    #[cfg(feature = "contextual-sea-orm")]
    fn sea_orm_conversion_is_available_when_enabled() {
        assert_from::<sea_orm::DbErr>();
    }

    #[test]
    #[cfg(feature = "contextual-redis")]
    fn redis_conversions_are_available_when_enabled() {
        assert_from::<deadpool_redis::PoolError>();
        assert_from::<redis::RedisError>();
    }

    #[test]
    #[cfg(feature = "contextual-cache")]
    fn cache_conversion_is_available_when_enabled() {
        assert_from::<multi_level_cache::CacheError>();
    }

    #[test]
    #[cfg(feature = "contextual-serde")]
    fn serde_conversion_is_available_when_enabled() {
        assert_from::<serde_json::Error>();
    }

    #[test]
    #[cfg(feature = "contextual-tokio")]
    fn tokio_conversions_are_available_when_enabled() {
        assert_from::<tokio::sync::AcquireError>();
        assert_from::<tokio::task::JoinError>();
    }
}
