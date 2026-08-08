use crate::{error::AppError, ext::ResultErrExt};
use axum::extract::{FromRequestParts, Path};
use serde::de::DeserializeOwned;
use std::ops::Deref;

/// 带错误日志的路径参数提取器
///
/// 包装 axum 的 `Path<T>`，在反序列化失败时通过 `tracing::warn!` 记录日志，
/// 并返回 `AppError::BadRequest` 而非 axum 默认的 `PathRejection`，
/// 确保错误响应格式与项目内其他错误一致。
///
/// # 用法
/// ```ignore
/// async fn handler(ValidatedPath(photo_id): ValidatedPath<PhotoId>) -> Result<...> {
///     // photo_id 已经是 PhotoId 类型
/// }
/// ```
pub struct ValidatedPath<T>(pub T);

impl<T, S> FromRequestParts<S> for ValidatedPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<T>::from_request_parts(parts, state)
            .await
            .trace_warn_bad_request(
                "validated_path_parse_error",
                "解析路径参数失败",
                "解析路径参数失败",
            )?;

        Ok(ValidatedPath(value))
    }
}

impl<T> Deref for ValidatedPath<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> AsRef<T> for ValidatedPath<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}
