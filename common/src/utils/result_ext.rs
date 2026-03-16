use crate::error::AppError;
use crate::r::R;
use tracing::error;

pub trait ResultExt<T, E> {
    fn map_internal_err(self, context: &'static str) -> Result<T, AppError>;
    fn map_bad_request_err(self, context: &'static str) -> Result<T, AppError>;

    fn to_bad_request_error(self) -> Result<T, AppError>
    where
        E: std::fmt::Display;

    fn into_ok_res(self) -> Result<R<T>, AppError>
    where
        Self: Sized,
        E: Into<AppError>,
        T: serde::Serialize;
}

impl<T, E: std::fmt::Debug> ResultExt<T, E> for Result<T, E> {
    #[inline]
    fn map_internal_err(self, context: &'static str) -> Result<T, AppError> {
        self.map_err(|e| {
            error!(target:"logs", "内部错误: {} \n {:?}", context, e);
            AppError::InternalServerError
        })
    }

    #[inline]
    fn map_bad_request_err(self, context: &'static str) -> Result<T, AppError> {
        self.map_err(|_| {
            AppError::bad_request(context)
        })
    }

    /// TODO static str
    #[inline]
    fn to_bad_request_error(self) -> Result<T, AppError>
    where
        E: std::fmt::Display,
    {
        self.map_err(|e| {
            AppError::BadRequest(e.to_string().into())
        })
    }

    #[inline]
    fn into_ok_res(self) -> Result<R<T>, AppError>
    where
        E: Into<AppError>,
        T: serde::Serialize,
    {
        self.map(R::ok).map_err(|e| e.into())
    }
}

pub trait ToOkExt {
    fn into_ok<E>(self) -> Result<Self, E>
    where
        Self: Sized;

    // 针对你常用的 AppError 进一步简化
    fn ok_res(self) -> Result<Self, AppError>
    where
        Self: Sized;
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