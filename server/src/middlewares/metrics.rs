//! HTTP 请求级指标中间件（RED：请求量、错误、耗时）
//!
//! 记录请求总量、耗时分布与在途请求，指标归 `server.http.*` 系统指标体系。
//! `route` 标签取自 `axum::extract::MatchedPath`（路由 pattern，不包含真实 ID），
//! 未匹配（404）时回退为 `unmatched`。
use std::time::Instant;

use common::time::Duration;

use axum::extract::MatchedPath;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use common::utils::GaugeGuard;

/// 记录请求耗时, 状态码和路由指标.
pub async fn metrics_middleware(request: Request, next: Next) -> Response {
    let method = request.method().to_string();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| "unmatched".to_owned());
    let start = Instant::now();

    // 在途请求守卫：进入 +1，响应返回（drop）时 -1
    let _in_flight = GaugeGuard::start("server.http.in_flight");

    let response = next.run(request).await;

    let status_class = format!("{}xx", response.status().as_u16() / 100);
    record(
        &method,
        &route,
        classify_module(&route),
        &status_class,
        start.elapsed(),
    );
    response
}

/// 按路由前缀归类模块（低基数：模块数量固定）。
/// 未知前缀（`/health`、`/hello`、未匹配等）归 `other`。
fn classify_module(route: &str) -> &'static str {
    match route.split('/').nth(1) {
        Some("auth") => "auth",
        Some("user") => "user",
        Some("photo") => "photo",
        Some("admin") => "audit", // /admin/audits
        _ => "other",
    }
}

/// 写入单次请求的聚合指标.
fn record(method: &str, route: &str, module: &str, status_class: &str, elapsed: Duration) {
    metrics::counter!(
        "server.http.requests_total",
        "method" => method.to_string(),
        "route" => route.to_string(),
        "module" => module.to_string(),
        "status_class" => status_class.to_string()
    )
    .increment(1);

    metrics::histogram!(
        "server.http.duration_seconds",
        "method" => method.to_string(),
        "route" => route.to_string(),
        "module" => module.to_string()
    )
    .record(elapsed.as_secs_f64());
}

#[cfg(test)]
mod tests {
    use super::classify_module;

    #[test]
    fn classifies_known_modules_by_route_prefix() {
        assert_eq!(classify_module("/auth/login"), "auth");
        assert_eq!(classify_module("/auth/token"), "auth");
        assert_eq!(classify_module("/user/me"), "user");
        assert_eq!(classify_module("/photo/:id"), "photo");
        assert_eq!(classify_module("/photo/face/:id"), "photo");
        assert_eq!(classify_module("/admin/audits"), "audit");
    }

    #[test]
    fn falls_back_to_other_for_system_routes() {
        assert_eq!(classify_module("/health"), "other");
        assert_eq!(classify_module("/hello"), "other");
        assert_eq!(classify_module("unmatched"), "other");
        assert_eq!(classify_module(""), "other");
    }
}
