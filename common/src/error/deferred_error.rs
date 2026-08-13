use std::fmt::{Debug, Display, Formatter};
use std::panic::Location;

use tracing::Level;

use crate::{error::AppError, ext::error_ext::base::log_and_map_at};

/// 尚未记录日志的应用错误。
///
/// mapper、repo 和基础设施层使用该错误保留底层原因；错误进入 service 后，
/// 由 `From<DeferredError> for AppError` 在 service 的 `?` 调用点统一记录。
pub struct DeferredError(Box<dyn DeferredReport>);

trait DeferredReport: Debug + Send + Sync {
    fn app_error(&self) -> &AppError;
    fn emit(self: Box<Self>, location: &'static Location<'static>) -> AppError;
}

#[derive(Debug)]
struct Report<E> {
    level: Level,
    reason: &'static str,
    context: &'static str,
    source: E,
    app_error: AppError,
}

impl<E> DeferredReport for Report<E>
where
    E: Debug + Send + Sync + 'static,
{
    fn app_error(&self) -> &AppError {
        &self.app_error
    }

    fn emit(self: Box<Self>, location: &'static Location<'static>) -> AppError {
        log_and_map_at(
            self.level,
            self.reason,
            self.context,
            Some(&self.source),
            self.app_error,
            location,
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

impl DeferredReport for ReportWithoutSource {
    fn app_error(&self) -> &AppError {
        &self.app_error
    }

    fn emit(self: Box<Self>, location: &'static Location<'static>) -> AppError {
        log_and_map_at(
            self.level,
            self.reason,
            self.context,
            None,
            self.app_error,
            location,
        )
    }
}

impl DeferredError {
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

impl Debug for DeferredError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for DeferredError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.0.app_error(), f)
    }
}

impl std::error::Error for DeferredError {}

impl From<sea_orm::DbErr> for DeferredError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::error("db_err", "数据库错误", error, AppError::InternalServerError)
    }
}

impl From<deadpool_redis::PoolError> for DeferredError {
    fn from(error: deadpool_redis::PoolError) -> Self {
        Self::error(
            "redis_err",
            "Redis错误",
            error,
            AppError::InternalServerError,
        )
    }
}

impl From<redis::RedisError> for DeferredError {
    fn from(error: redis::RedisError) -> Self {
        Self::error(
            "redis_err",
            "Redis错误",
            error,
            AppError::InternalServerError,
        )
    }
}

impl From<multi_level_cache::CacheError> for DeferredError {
    fn from(error: multi_level_cache::CacheError) -> Self {
        Self::warn(
            "cache_err",
            "缓存错误",
            error,
            AppError::InternalServerError,
        )
    }
}

impl From<serde_json::Error> for DeferredError {
    fn from(error: serde_json::Error) -> Self {
        Self::warn(
            "serde_json_error",
            "serde_json错误",
            error,
            AppError::InternalServerError,
        )
    }
}

#[cfg(feature = "tokio")]
impl From<tokio::sync::AcquireError> for DeferredError {
    fn from(error: tokio::sync::AcquireError) -> Self {
        Self::error(
            "tokio_semaphore_error",
            "信号量错误",
            error,
            AppError::InternalServerError,
        )
    }
}

#[cfg(feature = "tokio")]
impl From<tokio::task::JoinError> for DeferredError {
    fn from(error: tokio::task::JoinError) -> Self {
        Self::error(
            "tokio_join_error",
            "Tokio 任务执行失败",
            error,
            AppError::InternalServerError,
        )
    }
}

impl From<DeferredError> for AppError {
    #[track_caller]
    fn from(error: DeferredError) -> Self {
        error.0.emit(Location::caller())
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

    fn service_boundary(error: DeferredError) -> (u32, crate::Result<()>) {
        let expected_line = line!() + 2;
        let result = (|| -> crate::Result<()> {
            Err::<(), DeferredError>(error)?;
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
            service_boundary(DeferredError::error(
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
            output.contains("common/src/error/deferred_error.rs"),
            "unexpected log output: {output}"
        );
        assert!(
            output.contains(&format!("caller.line={expected_line}")),
            "unexpected log output: {output}"
        );
        assert_eq!(output.matches("reason=\"db_err\"").count(), 1);
    }

    #[test]
    fn deferred_error_is_pointer_sized() {
        assert_eq!(
            std::mem::size_of::<DeferredError>(),
            2 * std::mem::size_of::<usize>()
        );
    }
}
