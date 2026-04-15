use axum::{
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::de::DeserializeOwned;
use std::ops::Deref;
use validator::{Validate, ValidationErrors};

pub struct ValidatedJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
            .await
            .map_err(|err| {
                tracing::error!("Failed to read body: {}", err);
                StatusCode::BAD_REQUEST.into_response()
            })?;

        let value: T = serde_json::from_slice(&bytes).map_err(|err| {
            tracing::error!("Failed to parse JSON: {}", err);
            let msg = format!("JSON parse error: {}", err);
            (
                StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({ "error": msg })),
            )
                .into_response()
        })?;

        value.validate().map_err(|err: ValidationErrors| {
            tracing::error!("Validation failed: {}", err);
            let msg = err
                .field_errors()
                .into_iter()
                .map(|(field, errors)| {
                    let messages: Vec<String> = errors.iter().filter_map(|e| e.message.as_ref().map(|m| m.to_string())).collect();
                    format!("{}: {}", field, messages.join(", "))
                })
                .collect::<Vec<_>>()
                .join("; ");
            (
                StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({ "error": msg })),
            )
                .into_response()
        })?;

        Ok(ValidatedJson(value))
    }
}

impl<T> Deref for ValidatedJson<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
