use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct R<T> {
    pub code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> R<T>
where
    T: Serialize,
{
    pub fn ok(data: T) -> Self {
        Self {
            code: 200,
            msg: None,
            data: Some(data),
        }
    }
}

impl R<()> {
    pub fn err(code: u16, msg: &str) -> Self {
        Self {
            code,
            msg: Some(msg.to_string()),
            data: None,
        }
    }
}

impl<T> IntoResponse for R<T>
where
    T: Serialize,
{
    fn into_response(self) -> axum::response::Response {
        let status = StatusCode::from_u16(self.code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self)).into_response()
    }
}
