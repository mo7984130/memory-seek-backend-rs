use axum::{extract::Request, middleware::Next, response::Response};
use common::extractors::ClientIp;
use tracing::Instrument;

/// 为请求创建 tracing span 并关联后续处理过程.
pub async fn tracing_span(request: Request, next: Next) -> Response {
    let (parts, body) = request.into_parts();
    let method = parts.method.clone();
    let uri = parts.uri.clone();
    // 由外层 client_ip / trace_id 中间件注入，此处填充到 span 便于日志审计关联
    let client_ip = parts
        .extensions
        .get::<ClientIp>()
        .map(|ip| ip.0.to_string());
    let trace_id = parts.extensions.get::<String>().cloned();
    let request = Request::from_parts(parts, body);

    let span = tracing::info_span!(
        "request",
        method = %method,
        uri = %uri,
        client_ip = tracing::field::Empty,
        trace_id = tracing::field::Empty
    );
    if let Some(ip) = client_ip {
        span.record("client_ip", ip.as_str());
    }
    if let Some(tid) = trace_id {
        span.record("trace_id", tid.as_str());
    }
    next.run(request).instrument(span).await
}
