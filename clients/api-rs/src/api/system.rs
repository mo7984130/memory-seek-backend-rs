//! 系统级端点(非业务契约):Prometheus 指标抓取等。
//!
//! 这些端点返回非 `R` 包壳内容(如 `text/plain`),故不经类型化封装,
//! 直接返回原始文本。

use crate::client::Client;
use crate::error::{ApiError, Result};
use crate::transport::Method;

/// 健康检查。`GET /health`(公开路由)。
///
/// 返回 `text/plain` 而非 `R` 包壳,故直接返回响应体文本;非 2xx 报
/// [`ApiError::HttpStatus`]。
pub async fn health(client: &Client) -> Result<String> {
    let response = client.raw(Method::GET, "/health").no_auth().send().await?;
    if !response.is_success() {
        return Err(ApiError::HttpStatus {
            code: response.status.as_u16(),
            msg: "健康检查失败".to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&response.body).into_owned())
}

/// 抓取 Prometheus 指标文本。`GET /metrics`(公开路由)。
///
/// 返回 `text/plain` 而非 `R` 包壳,故直接返回响应体文本;非 2xx 报
/// [`ApiError::HttpStatus`]。
pub async fn metrics(client: &Client) -> Result<String> {
    let response = client.raw(Method::GET, "/metrics").no_auth().send().await?;
    if !response.is_success() {
        return Err(ApiError::HttpStatus {
            code: response.status.as_u16(),
            msg: "指标抓取失败".to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&response.body).into_owned())
}
