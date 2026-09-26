//! HTTP 请求级指标中间件（RED：请求量、错误、耗时）
//!
//! 记录请求总量、耗时分布与在途请求，指标归 `server.http.*` 系统指标体系。
//! `route` 标签取自 `axum::extract::MatchedPath`（路由 pattern，不包含真实 ID），
//! 未匹配（404）时回退为 `unmatched`。
use std::time::Instant;

use common_core::time::Duration;

use axum::extract::MatchedPath;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use common_metrics::GaugeGuard;

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
    let mut segments = route.split('/');
    let _ = segments.next(); // 前导空串
    match segments.next() {
        Some("auth") => "auth",
        Some("user") => "user",
        Some("visual") => "visual",
        // /admin/backup/* → backup；其余 /admin/*（如 /admin/audits）→ audit
        Some("admin") => match segments.next() {
            Some("backup") => "backup",
            _ => "audit",
        },
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
        assert_eq!(classify_module("/visual/:id"), "visual");
        assert_eq!(classify_module("/visual/face/:id"), "visual");
        assert_eq!(classify_module("/admin/audits"), "audit");
        assert_eq!(classify_module("/admin/backup/trigger"), "backup");
        assert_eq!(classify_module("/admin/backup/restore"), "backup");
    }

    #[test]
    fn falls_back_to_other_for_system_routes() {
        assert_eq!(classify_module("/health"), "other");
        assert_eq!(classify_module("/hello"), "other");
        assert_eq!(classify_module("unmatched"), "other");
        assert_eq!(classify_module(""), "other");
    }
}
