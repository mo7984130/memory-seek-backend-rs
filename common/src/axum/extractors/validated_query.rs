use crate::{
    axum::extractors::handle_validation_error,
    error::{AppError, contextual::ext::ResultContextualExt},
};
use axum::extract::{FromRequestParts, Query};
use serde::de::DeserializeOwned;
use std::ops::Deref;
use validator::Validate;

pub struct ValidatedQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for ValidatedQuery<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .context_warn(
                "validated_query_parse_error",
                "解析查询参数失败",
                AppError::bad_request("解析查询参数失败"),
            )?;

        value.validate().map_err(handle_validation_error)?;

        Ok(ValidatedQuery(value))
    }
}

impl<T> Deref for ValidatedQuery<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> AsRef<T> for ValidatedQuery<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}
