//! HTTP 请求级指标中间件（RED：请求量、错误、耗时）
//!
//! 记录请求总量、耗时分布与在途请求，指标归 `server.http.*` 系统指标体系。
//! `route` 标签取自 `axum::extract::MatchedPath`（路由 pattern，不包含真实 ID），
//! 未匹配（404）时回退为 `unmatched`。
use std::time::Instant;

use axum::extract::MatchedPath;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use common::utils::GaugeGuard;

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
    record(&method, &route, &status_class, start.elapsed());
    response
}

#[cfg(feature = "metrics")]
fn record(method: &str, route: &str, status_class: &str, elapsed: std::time::Duration) {
    metrics::counter!(
        "server.http.requests_total",
        "method" => method.to_string(),
        "route" => route.to_string(),
        "status_class" => status_class.to_string()
    )
    .increment(1);

    metrics::histogram!(
        "server.http.duration_seconds",
        "method" => method.to_string(),
        "route" => route.to_string()
    )
    .record(elapsed.as_secs_f64());
}

#[cfg(not(feature = "metrics"))]
fn record(_method: &str, _route: &str, _status_class: &str, _elapsed: std::time::Duration) {}
