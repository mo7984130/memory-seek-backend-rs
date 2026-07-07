use axum::http::{HeaderName, Method};
use tower_http::cors::{Any, CorsLayer};

/// 创建 CORS 中间件
pub fn layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static("authorization"),
            HeaderName::from_static("x-trace-id"),
            HeaderName::from_static("x-user-id"),
            HeaderName::from_static("x-refresh-token"),
        ])
}
