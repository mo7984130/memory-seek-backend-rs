use common::extractors::ClientIp;
use axum::{
    body::Body,
    extract::FromRequestParts,
    http::{HeaderValue, Request},
    middleware::Next,
};
use tracing::Instrument;

/// 链路追踪中间件
///
/// 为每个请求生成唯一 `trace_id`（UUID v4），提取客户端 IP，
/// 创建包含 method、uri、trace_id、client_ip 的 tracing span，
/// 并在响应头中添加 `X-Trace-Id`。
///
/// # 参数
/// - `req`: HTTP 请求
/// - `next`: 下一个中间件/处理器
///
/// # 返回
/// 返回带有 `X-Trace-Id` 响应头的下游响应
pub async fn trace_id_middleware(req: Request<Body>, next: Next) -> axum::response::Response {
    let trace_id = uuid::Uuid::new_v4().to_string();

    // 拆分 Request 为 parts 和 body
    let (mut parts, body) = req.into_parts();

    // 使用 ClientIp 提取器提取 IP（支持降级逻辑）
    let client_ip = ClientIp::from_request_parts(&mut parts, &())
        .await
        .map(|ip| ip.0.to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    // 重新组装 Request
    let req = Request::from_parts(parts, body);

    let span = tracing::info_span!(
        "http_request",
        method = %req.method(),
        uri = %req.uri(),
        trace_id = %trace_id,
        client_ip = %client_ip
    );

    let mut response = next.run(req).instrument(span).await;
    if let Ok(val) = HeaderValue::from_str(&trace_id) {
        response.headers_mut().insert("X-Trace-Id", val);
    }

    response
}
