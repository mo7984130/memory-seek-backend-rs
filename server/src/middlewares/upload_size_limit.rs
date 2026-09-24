use crate::state::AppState;
use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use common::error::AppError;
use std::sync::Arc;

/// 请求体大小预检中间件
///
/// 对携带 `Content-Length` 且超过上限的请求直接返回 413,
/// 避免大文件继续传输到业务层才被拒绝(axum 默认 2MB 限制读流中断,
/// 表现为请求体读取失败)。未知长度(chunked)请求由
/// `DefaultBodyLimit` + 各 controller 的错误映射兜底。
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
