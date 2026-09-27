use crate::extractors::handle_validation_error;
use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
};
use common_core::error::{AppError, ContextualError, contextual::ext::ResultContextualExt};
use serde::de::DeserializeOwned;
use std::ops::Deref;
use validator::Validate;

pub struct ValidatedJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, AppError> {
        let mut bytes = Bytes::from_request(req, state).await.context_warn(
            "validated_json_read_error",
            "读取请求体失败",
            AppError::bad_request("请求体读取失败"),
        )?;
        if bytes.is_empty() {
            bytes = Bytes::from_static(b"{}");
        }

        let value: T = serde_json::from_slice(&bytes).map_err(|error| {
            ContextualError::warn(
                "validated_json_deserialize_error",
                "解析请求体失败",
                error,
                AppError::bad_request("请求体格式错误"),
            )
        })?;

        value.validate().map_err(handle_validation_error)?;

        Ok(Self(value))
    }
}

impl<T> Deref for ValidatedJson<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> AsRef<T> for ValidatedJson<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}
