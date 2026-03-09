use axum::{response::IntoResponse, Json};
use serde::Serialize;
use serde_with::skip_serializing_none;

#[derive(Serialize)]
#[skip_serializing_none]
#[serde(rename_all = "camelCase")]
pub struct R<T> {
    pub code: u16,
    pub msg: Option<String>,
    pub data: Option<T>
}

impl<T> R<T>
where
    T: Serialize,
{
    pub fn ok(data: T) -> Self {
        Self {
            code: 200,
            msg: None,
            data: Some(data)
        }
    }
}

impl R<()> {
    pub fn err(code: u16, msg: &str) -> Self {
        Self {
            code,
            msg: Some(msg.to_string()),
            data: None
        }
    }
}

impl<T> IntoResponse for R<T>
where
    T: Serialize
{
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}