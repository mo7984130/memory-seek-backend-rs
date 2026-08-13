use crate::{
    error::AppError,
    ext::{log_warn, log_warn_with_source},
    extractors::validated_json::format_validation_errors,
};
use axum::extract::{FromRequestParts, Query};
use serde::de::DeserializeOwned;
use std::ops::Deref;
use validator::{Validate, ValidationErrors};

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
        let Query(value) =
            Query::<T>::from_request_parts(parts, state)
                .await
                .map_err(|source| {
                    log_warn_with_source(
                        "validated_query_parse_error",
                        "解析查询参数失败",
                        source,
                        AppError::bad_request("解析查询参数失败"),
                    )
                })?;

        value.validate().map_err(|err: ValidationErrors| {
            let msg = format_validation_errors(&err);
            log_warn(
                "validated_query_validate_error",
                "校验失败",
                AppError::bad_request(msg),
            )
        })?;

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
