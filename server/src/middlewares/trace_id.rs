use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

/// 请求追踪 ID 中间件
///
/// 为每个请求生成唯一的 trace_id，并添加到响应头中
pub async fn trace_id_middleware(
    request: Request,
    next: Next,
) -> Response {
    let trace_id = Uuid::new_v4().to_string();

    let mut request = request;
    request.extensions_mut().insert(trace_id.clone());

    let mut response = next.run(request).await;

    response.headers_mut().insert(
        "x-trace-id",
        trace_id.parse().unwrap(),
    );

    response
}
