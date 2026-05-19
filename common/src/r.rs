use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

/// 统一 API 响应格式
///
/// 所有接口使用此结构体作为响应载体，JSON 序列化后字段名为驼峰命名。
/// 成功时包含 `code` 和 `data`，失败时包含 `code` 和 `msg`。
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
    /// 创建成功响应
    ///
    /// # 参数
    /// - `data`: 响应数据，将被序列化为 JSON
    ///
    /// # 返回
    /// 返回 `code` 为 200 且包含 `data` 的 `R<T>`
    pub fn ok(data: T) -> Self {
        Self {
            code: 200,
            msg: None,
            data: Some(data),
        }
    }
}

impl R<()> {
    /// 创建错误响应
    ///
    /// # 参数
    /// - `code`: HTTP 状态码
    /// - `msg`: 错误描述信息
    ///
    /// # 返回
    /// 返回指定 `code` 和 `msg` 且无 `data` 的 `R<()>`
    pub fn err(code: u16, msg: &str) -> Self {
        Self {
            code,
            msg: Some(msg.to_string()),
            data: None,
        }
    }
}

/// 将 `R<T>` 转换为 HTTP 响应
///
/// 使用 `code` 字段作为 HTTP 状态码，若状态码无效则回退到 500。
impl<T> IntoResponse for R<T>
where
    T: Serialize,
{
    fn into_response(self) -> axum::response::Response {
        let status = StatusCode::from_u16(self.code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self)).into_response()
    }
}
