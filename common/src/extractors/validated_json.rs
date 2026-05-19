use axum::{
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::de::DeserializeOwned;
use std::ops::Deref;
use validator::{Validate, ValidationErrors};

/// 带自动验证的 JSON 请求体提取器
///
/// 组合了 JSON 反序列化和 `validator` 校验，替代 axum 原生的 `Json` 提取器。
/// 当请求体解析失败或校验不通过时，返回 400 状态码和错误详情。
pub struct ValidatedJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = Response;

    /// 从请求体解析 JSON 并执行校验
    ///
    /// # 参数
    /// - `req`: HTTP 请求，从中读取 body 并反序列化为 `T`
    /// - `state`: axum 应用状态
    ///
    /// # 返回
    /// 返回校验通过的 `ValidatedJson<T>` 包装值
    ///
    /// # 错误
    /// - `400 Bad Request`: body 读取失败、JSON 解析失败或字段校验不通过
    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
            .await
            .map_err(|err| {
                tracing::warn!("读取body错误: {}", err);
                StatusCode::BAD_REQUEST.into_response()
            })?;

        let value: T = serde_json::from_slice(&bytes).map_err(|err| {
            tracing::warn!("解析JSON错误: {}", err);
            let msg = format!("解析JSON错误: {}", err);
            (
                StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({ "error": msg })),
            )
                .into_response()
        })?;

        value.validate().map_err(|err: ValidationErrors| {
            tracing::warn!("效验失败: {}", err);
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

/// 解引用到内部类型 `T`，方便直接调用 `T` 的方法
impl<T> Deref for ValidatedJson<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
