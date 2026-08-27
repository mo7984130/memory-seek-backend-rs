use crate::error::{AppError, contextual::ext::ResultContextualExt};
use axum::extract::{FromRequestParts, Path};
use serde::de::DeserializeOwned;
use std::ops::Deref;

pub struct ValidatedPath<T>(pub T);

impl<T, S> FromRequestParts<S> for ValidatedPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<T>::from_request_parts(parts, state)
            .await
            .context_warn(
                "validated_path_parse_error",
                "解析路径参数失败",
                AppError::bad_request("解析路径参数失败"),
            )?;

        Ok(ValidatedPath(value))
    }
}

impl<T> Deref for ValidatedPath<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> AsRef<T> for ValidatedPath<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}
