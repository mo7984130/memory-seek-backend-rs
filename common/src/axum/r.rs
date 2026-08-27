use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

/// 统一 API 响应格式
///
/// 所有接口使用此结构体作为响应载体，JSON 序列化后字段名为驼峰命名。
/// 成功时包含 `code` 和 `data`，失败时包含 `code` 和 `msg`。
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "common/"))]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_creates_success_response() {
        let r = R::ok("hello");
        assert_eq!(r.code, 200);
        assert!(r.msg.is_none());
        assert_eq!(r.data, Some("hello"));
    }

    #[test]
    fn ok_with_integer_data() {
        let r = R::ok(42i32);
        assert_eq!(r.code, 200);
        assert!(r.msg.is_none());
        assert_eq!(r.data, Some(42));
    }

    #[test]
    fn err_creates_error_response() {
        let r: R<()> = R::err(400, "bad request");
        assert_eq!(r.code, 400);
        assert_eq!(r.msg.as_deref(), Some("bad request"));
        assert!(r.data.is_none());
    }

    #[test]
    fn err_with_401() {
        let r: R<()> = R::err(401, "unauthorized");
        assert_eq!(r.code, 401);
        assert_eq!(r.msg.as_deref(), Some("unauthorized"));
        assert!(r.data.is_none());
    }

    #[test]
    fn ok_serializes_to_json() {
        let r = R::ok("test_value");
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["code"], 200);
        assert!(json.get("msg").is_none()); // skipped by skip_serializing_if
        assert_eq!(json["data"], "test_value");
    }

    #[test]
    fn err_serializes_to_json() {
        let r: R<()> = R::err(404, "not found");
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["code"], 404);
        assert_eq!(json["msg"], "not found");
        assert!(json.get("data").is_none()); // skipped by skip_serializing_if
    }

    #[test]
    fn ok_with_complex_data_serializes() {
        #[derive(Serialize, Debug, PartialEq)]
        #[serde(rename_all = "camelCase")]
        struct UserDTO {
            id: String,
            nickname: String,
        }

        let r = R::ok(UserDTO {
            id: "123".to_string(),
            nickname: "Alice".to_string(),
        });
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["code"], 200);
        assert_eq!(json["data"]["id"], "123");
        assert_eq!(json["data"]["nickname"], "Alice");
    }

    #[test]
    fn r_unit_with_null_data() {
        let r: R<()> = R::ok(());
        assert_eq!(r.code, 200);
        assert!(r.msg.is_none());
        assert_eq!(r.data, Some(()));
        // Serialization: Some(()) serializes as `null` in JSON
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["code"], 200);
        assert_eq!(json["data"], serde_json::Value::Null);
    }

    #[test]
    fn r_unit_err_serialization() {
        let r: R<()> = R::err(500, "internal error");
        let json = serde_json::to_value(&r).unwrap();
        // Verify camelCase field names
        assert!(json.get("code").is_some());
        assert!(json.get("msg").is_some());
        assert!(json.get("data").is_none());
        assert!(json.get("Code").is_none()); // should be camelCase, not PascalCase
    }
}
