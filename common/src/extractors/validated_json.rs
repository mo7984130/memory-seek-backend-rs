use crate::{
    error::AppError,
    ext::{ResultErrExt, log_warn},
};
use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
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
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, AppError> {
        // Content-Type 检查（可选，按需保留）
        let content_type_ok = req
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("application/json"));

        if !content_type_ok {
            return Err(AppError::bad_request(
                "Content-Type 必须为 application/json",
            ));
        }

        // 直接拿 Bytes，body 大小限制已由 DefaultBodyLimit 中间件保证
        // axum 内部会一次性读完 stream 拼成一个连续的 Bytes，没有多余的中间层拷贝
        let bytes = Bytes::from_request(req, state)
            .await
            .trace_warn_bad_request(
                "validated_json_read_error",
                "读取请求体失败",
                " 请求体读取失败",
            )?;

        let value: T = serde_json::from_slice(&bytes).map_err(|e| {
            log_warn(
                "validated_json_parse_error",
                "解析JSON错误",
                AppError::bad_request(format!("JSON解析错误: {}", e)),
            )
        })?;
        value.validate().map_err(|err: ValidationErrors| {
            let msg = format_validation_errors(&err);
            log_warn(
                "validated_json_validate_error",
                "校验失败",
                AppError::bad_request(msg),
            )
        })?;

        Ok(ValidatedJson(value))
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

impl<T> Deref for ValidatedJson<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> AsRef<T> for ValidatedJson<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}
