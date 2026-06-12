use axum::extract::{FromRequestParts, Query};
use serde::de::DeserializeOwned;
use std::ops::Deref;
use validator::Validate;

use crate::{
    error::AppError,
    ext::{ResultErrExt, log_warn},
};

/// 带自动验证的 Query 参数提取器
///
/// 组合了 Query 反序列化和 `validator` 校验，替代 axum 原生的 `Query` 提取器。
/// 当查询参数解析失败或校验不通过时，返回 400 状态码和错误详情。
pub struct ValidatedQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for ValidatedQuery<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = AppError;

    /// 从请求 URI 解析查询参数并执行校验
    ///
    /// # 参数
    /// - `parts`: HTTP 请求的部分信息，从中提取查询字符串
    /// - `state`: axum 应用状态
    ///
    /// # 返回
    /// 返回校验通过的 `ValidatedQuery<T>` 包装值
    ///
    /// # 错误
    /// - `400 Bad Request`: 查询参数解析失败或字段校验不通过
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .trace_warn(
                "validated_query_parse_err",
                "解析查询参数失败",
                AppError::bad_request("解析查询参数失败"),
            )?;

        value.validate().map_err(|err| {
            let msg = err
                .field_errors()
                .into_iter()
                .map(|(field, errors)| {
                    let messages: Vec<String> = errors
                        .iter()
                        .filter_map(|e| e.message.as_ref().map(|m| m.to_string()))
                        .collect();
                    format!("{}: {}", field, messages.join(", "))
                })
                .collect::<Vec<_>>()
                .join("; ");
            log_warn(
                "validated_query_validate_err",
                "效验失败",
                err,
                AppError::bad_request(msg),
            )
        })?;

        Ok(ValidatedQuery(value))
    }
}

/// 解引用到内部类型 `T`，方便直接调用 `T` 的方法
impl<T> Deref for ValidatedQuery<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
