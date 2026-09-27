//! 收尾(teardown)复用的删除动作。
//!
//! 语义:
//! - 幂等: 资源已不存在(404)视为收尾完成, 不算失败(本轮可能根本没创建成功);
//! - 其余非 200 视为收尾失败并上报(框架单列 `teardown_failures`, 不影响通过率);
//! - 只删本次轮次/任务自己创建的产物, 绝不触碰种子数据。

use memseek_test::ctxlibs::http_client::{HttpError, reqwest};
use serde_json::Value;

use crate::context::Context;

/// 删除资源(可带 JSON body), 404 视为已完成。
pub async fn delete(
    ctx: &Context,
    path: &str,
    auth: Option<&str>,
    body: Option<Value>,
) -> Result<(), HttpError> {
    let mut request = ctx.client.request(reqwest::Method::DELETE, path);
    if let Some(auth) = auth {
        request = request.header("Authorization", auth);
    }
    if let Some(body) = body {
        request = request.json_unwrap(&body);
    }

    let url = ctx.client.resolve(path)?;
    let response = request.send().await?;
    let status = response.status();
    if status.is_success() || status == reqwest::StatusCode::NOT_FOUND {
        return Ok(());
    }
    Err(HttpError::status(status, reqwest::Method::DELETE, &url))
}

/// 把非 HTTP 侧的收尾失败(如直连数据库删除)包装成场景错误上报。
///
/// 原因文本随 `response_body` 保留, 同时调用方应 `tracing::warn!` 落日志:
/// 框架的错误明细只按分类计数, 不带原因。
pub fn internal(reason: impl Into<String>) -> HttpError {
    HttpError::Status {
        status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
        method: reqwest::Method::DELETE,
        url: "cleanup://internal".to_string(),
        request_body: None,
        response_body: Some(Err(reason.into())),
    }
}
