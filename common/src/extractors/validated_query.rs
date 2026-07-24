use crate::{
    error::AppError,
    ext::{ResultErrExt, log_warn},
    // 建议把 format_validation_errors 提到公共模块后从这里 use 进来
    // extractors::json::format_validation_errors,
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
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .trace_warn_bad_request(
                "validated_query_parse_error",
                "解析查询参数失败",
                "解析查询参数失败",
            )?;

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

fn format_validation_errors(errors: &ValidationErrors) -> String {
    errors
        .field_errors()
        .iter()
        .map(|(field, errs)| {
            let messages: Vec<String> = errs
                .iter()
                .filter_map(|e| e.message.as_ref().map(|m| m.to_string()))
                .collect();
            if messages.is_empty() {
                format!("字段 '{}' 校验失败", field)
            } else {
                format!("{}: {}", field, messages.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
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
