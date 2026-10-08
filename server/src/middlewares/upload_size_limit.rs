use crate::state::AppState;
use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use common_core::error::AppError;
use std::sync::Arc;

/// 请求体大小预检中间件；对超过上限的 `Content-Length` 请求直接返回 413
pub async fn upload_size_limit(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let limit = state.max_upload_bytes;
    if let Some(len) = request
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        && len > limit
    {
        return AppError::PayloadTooLarge.into_response();
    }
    next.run(request).await
}
