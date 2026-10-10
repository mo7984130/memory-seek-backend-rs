//! 产品 API 调用统一入口:基于 `memory-seek-api-rs` 客户端。
//!
//! 场景的业务请求一律经这里构造的 api-rs `Client` 发出 —— 请求/响应 DTO 直接复用
//! 后端契约 crate,后端改字段/类型时 e2e 会在编译期报错。
//!
//! 基础设施(MailHog / Redis / S3 / 直连 DB)不经此处,仍走各自的直连。

use memory_seek_api::{HttpRequest, HttpResponse, Transport, TransportError};
use memseek_test::ctxlibs::http_client::reqwest;

/// 基于 `reqwest` 的 [`Transport`] 实现。
///
/// 超时/连接池等由调用方提供的 `reqwest::Client` 决定;此处只做方法/头/体的搬运,
/// 并把响应还原为与 HTTP 库无关的 [`HttpResponse`](含状态码、响应头与原始字节)。
/// 请求/响应头直接复用 `http::HeaderMap`(reqwest 原生类型),无需转换。
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// 用给定的 `reqwest::Client` 构造。
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl Transport for ReqwestTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut builder = self
            .client
            .request(request.method, &request.url)
            .headers(request.headers);
        if let Some(body) = request.body {
            builder = builder.body(body);
        }

        let response = builder.send().await?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.bytes().await?.to_vec();
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}
