use crate::{
    error::AppError,
    ext::{log_warn, log_warn_with_source},
};
use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
};
use serde::de::DeserializeOwned;
use std::ops::Deref;
use validator::{Validate, ValidationErrors, ValidationErrorsKind};

pub struct ValidatedJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, AppError> {
        // 先借用 headers(读取 body 会消费 req)
        let content_type_ok = req
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("application/json"));

        // 直接拿 Bytes，body 大小限制已由 DefaultBodyLimit 中间件保证
        // axum 内部会一次性读完 stream 拼成一个连续的 Bytes，没有多余的中间层拷贝
        let bytes = Bytes::from_request(req, state).await.map_err(|source| {
            log_warn_with_source(
                "validated_json_read_error",
                "读取请求体失败",
                source,
                AppError::bad_request(" 请求体读取失败"),
            )
        })?;

        // 空 body 时跳过 Content-Type 检查, 用空对象反序列化:
        // 带默认值的 DTO(如全字段可选的参数)直接得到默认值, 必填字段报 missing field;
        // 否则要求 Content-Type 为 application/json 后正常解析。
        let value: T = if bytes.is_empty() {
            serde_json::from_slice(b"{}").map_err(|e| {
                log_warn(
                    "validated_json_parse_error",
                    "解析JSON错误",
                    AppError::bad_request(format!("JSON解析错误: {}", format_serde_error(&e))),
                )
            })?
        } else {
            if !content_type_ok {
                return Err(AppError::bad_request(
                    "Content-Type 必须为 application/json",
                ));
            }

            serde_json::from_slice(&bytes).map_err(|e| {
                log_warn(
                    "validated_json_parse_error",
                    "解析JSON错误",
                    AppError::bad_request(format!("JSON解析错误: {}", format_serde_error(&e))),
                )
            })?
        };
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

/// 格式化 serde_json 解析错误。
///
/// 自定义反序列化抛出的错误(`Category::Data`,如 `TimeIdCursor` 解码失败)
/// 会被 serde_json 追加 `at line X column Y`,该位置信息对这类错误毫无意义,
/// 这里剥离掉;真正的 JSON 语法错误(`Category::Syntax`)保留位置便于排查。
pub(crate) fn format_serde_error(e: &serde_json::Error) -> String {
    let msg = e.to_string();
    if e.classify() == serde_json::error::Category::Data {
        msg.split(" at line ").next().unwrap_or(&msg).to_string()
    } else {
        msg
    }
}

/// 递归收集所有校验错误消息(含 `#[validate(nested)]` 产生的嵌套错误)
fn collect_messages(errors: &ValidationErrors, out: &mut Vec<String>) {
    for kind in errors.errors().values() {
        match kind {
            ValidationErrorsKind::Field(errs) => {
                out.extend(
                    errs.iter()
                        .filter_map(|e| e.message.as_ref().map(|m| m.to_string())),
                );
            }
            ValidationErrorsKind::Struct(inner) => collect_messages(inner, out),
            ValidationErrorsKind::List(items) => {
                for inner in items.values() {
                    collect_messages(inner, out);
                }
            }
        }
    }
}

/// 将校验错误格式化为可读的中文消息。
///
/// 字段自带的 `message` 已是完整的中文描述,直接输出;仅当缺失 message
/// 时才退化为 `字段 'X' 校验失败`。
pub(crate) fn format_validation_errors(errors: &ValidationErrors) -> String {
    let mut messages = Vec::new();
    collect_messages(errors, &mut messages);
    if messages.is_empty() {
        let mut fields = Vec::new();
        collect_field_paths(errors, "", &mut fields);
        if fields.is_empty() {
            "参数校验失败".to_string()
        } else {
            format!("字段 '{}' 校验失败", fields.join(", "))
        }
    } else {
        messages.join("; ")
    }
}

fn collect_field_paths(errors: &ValidationErrors, prefix: &str, out: &mut Vec<String>) {
    for (field, kind) in errors.errors() {
        let path = if prefix.is_empty() {
            field.to_string()
        } else {
            format!("{prefix}.{field}")
        };
        match kind {
            ValidationErrorsKind::Field(_) => out.push(path),
            ValidationErrorsKind::Struct(inner) => collect_field_paths(inner, &path, out),
            ValidationErrorsKind::List(items) => {
                for inner in items.values() {
                    collect_field_paths(inner, &path, out);
                }
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, serde::Deserialize, validator::Validate)]
    struct Param {
        #[validate(length(max = 5, message = "名称不能超过5个字符"))]
        name: String,
    }

    #[derive(Debug, serde::Deserialize, validator::Validate)]
    struct Outer {
        #[validate(nested)]
        inner: Param,
    }

    /// 模拟自定义反序列化错误(与 validated_newtype 反序列化校验相同的模式)
    #[derive(Debug)]
    struct Reject;

    impl<'de> serde::Deserialize<'de> for Reject {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct V;

            impl<'de> serde::de::Visitor<'de> for V {
                type Value = Reject;

                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("a rejected value")
                }

                fn visit_str<E: serde::de::Error>(self, _v: &str) -> Result<Reject, E> {
                    Err(E::custom("业务校验失败"))
                }
            }

            d.deserialize_str(V)
        }
    }

    #[test]
    fn test_format_serde_error_strips_position_for_custom_error() {
        let e = serde_json::from_str::<Reject>(r#""超长值""#).unwrap_err();
        assert_eq!(e.classify(), serde_json::error::Category::Data);
        let msg = format_serde_error(&e);
        assert_eq!(msg, "业务校验失败");
        assert!(!msg.contains("at line"));
    }

    #[test]
    fn test_format_serde_error_keeps_position_for_syntax_error() {
        let e = serde_json::from_str::<Param>(r#"{invalid json"#).unwrap_err();
        assert_eq!(e.classify(), serde_json::error::Category::Syntax);
        let msg = format_serde_error(&e);
        assert!(msg.contains("at line"), "msg: {msg}");
    }

    #[test]
    fn test_format_validation_errors_outputs_message_without_field_prefix() {
        let e = serde_json::from_str::<Param>(r#"{"name": "toolong"}"#).unwrap();
        let errors = e.validate().unwrap_err();
        assert_eq!(format_validation_errors(&errors), "名称不能超过5个字符");
    }

    #[test]
    fn test_format_validation_errors_collects_nested_messages() {
        let e = serde_json::from_str::<Outer>(r#"{"inner": {"name": "toolong"}}"#).unwrap();
        let errors = e.validate().unwrap_err();
        assert_eq!(format_validation_errors(&errors), "名称不能超过5个字符");
    }

    #[test]
    fn test_format_validation_errors_fallback_when_no_message() {
        #[derive(Debug, serde::Deserialize, validator::Validate)]
        struct NoMsg {
            #[validate(length(max = 5))]
            name: String,
        }
        let e = serde_json::from_str::<NoMsg>(r#"{"name": "toolong"}"#).unwrap();
        let errors = e.validate().unwrap_err();
        assert_eq!(format_validation_errors(&errors), "字段 'name' 校验失败");
    }

    #[cfg(feature = "tokio")]
    mod integration {
        use super::*;
        use crate::error::AppError;
        use axum::{body::Body, http::Request};

        /// 模拟 `validated_newtype!` 生成的类型
        #[derive(Debug, Clone, serde::Deserialize)]
        struct PersonName(String);

        impl validator::Validate for PersonName {
            fn validate(&self) -> Result<(), validator::ValidationErrors> {
                if self.0.len() > 64 {
                    let mut err = validator::ValidationError::new("too_long");
                    err.message = Some(std::borrow::Cow::Borrowed("人物名称长度不能超过64个字符"));
                    let mut errors = validator::ValidationErrors::new();
                    errors.add("value", err);
                    return Err(errors);
                }
                Ok(())
            }
        }

        #[derive(Debug, serde::Deserialize, validator::Validate)]
        #[serde(rename_all = "camelCase")]
        struct RenameParam {
            #[validate(nested)]
            new_name: PersonName,
        }

        #[tokio::test]
        async fn test_person_name_too_long_returns_clean_message() {
            let json = format!(r#"{{"newName": "{}"}}"#, "a".repeat(65));
            let req = Request::builder()
                .method("POST")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(json))
                .unwrap();
            let res = ValidatedJson::<RenameParam>::from_request(req, &()).await;
            let err = match res {
                Ok(_) => panic!("expected validation error"),
                Err(e) => e,
            };
            match err {
                AppError::BadRequest(msg) => {
                    assert_eq!(msg.as_ref(), "人物名称长度不能超过64个字符");
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }

        #[tokio::test]
        async fn test_valid_json_passes_through() {
            let json = r#"{"newName": "Alice"}"#;
            let req = Request::builder()
                .method("POST")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(json))
                .unwrap();
            let res = ValidatedJson::<RenameParam>::from_request(req, &()).await;
            assert!(res.is_ok());
        }

        #[tokio::test]
        async fn test_invalid_json_returns_parse_error() {
            let req = Request::builder()
                .method("POST")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{invalid"#))
                .unwrap();
            let res = ValidatedJson::<RenameParam>::from_request(req, &()).await;
            let err = match res {
                Ok(_) => panic!("expected parse error"),
                Err(e) => e,
            };
            match err {
                AppError::BadRequest(msg) => {
                    assert!(msg.as_ref().starts_with("JSON解析错误: "));
                    assert!(msg.as_ref().contains("at line"), "msg: {msg}");
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }
}
