use axum::{
    extract::{FromRequestParts, Request},
    middleware::Next,
    response::Response,
};
use common::extractors::ClientIp;

/// 提取客户端 IP 并注入请求扩展.
pub async fn client_ip_middleware(request: Request, next: Next) -> Response {
    let (mut parts, body) = request.into_parts();

    let client_ip = ClientIp::from_request_parts(&mut parts, &()).await.ok();

    let mut request = Request::from_parts(parts, body);

    if let Some(ip) = client_ip {
        request.extensions_mut().insert(ip);
    }

    next.run(request).await
}
