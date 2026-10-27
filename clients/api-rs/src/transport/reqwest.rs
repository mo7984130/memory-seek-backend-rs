//! 基于 [`reqwest`](https://docs.rs/reqwest) 的 [`Transport`] 实现
//! (feature `reqwest-transport`)。
//!
//! 只做「方法 / 头 / 体」的搬运:超时、连接池、TLS 等由调用方提供的
//! [`reqwest::Client`] 决定。请求 / 响应头直接复用 `http::HeaderMap`
//! (reqwest 原生类型),无需转换。
//!
//! 依赖调用方提供异步运行时(如 tokio)。

use std::path::Path;

use tokio::io::AsyncWriteExt as _;

use super::{DownloadResponse, HttpRequest, HttpResponse, Transport, TransportError};

/// 基于 [`reqwest::Client`] 的传输层实现。
pub struct ReqwestTransport {
    client: ::reqwest::Client,
}

impl ReqwestTransport {
    /// 用默认 [`reqwest::Client`] 构造。
    pub fn new() -> Self {
        Self {
            client: ::reqwest::Client::new(),
        }
    }

    /// 用给定的 [`reqwest::Client`] 构造(自定义超时 / 连接池 / TLS 等)。
    pub fn with_client(client: ::reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for ReqwestTransport {
    fn default() -> Self {
        Self::new()
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

    async fn download(
        &self,
        request: HttpRequest,
        dest: &Path,
    ) -> Result<DownloadResponse, TransportError> {
        let mut builder = self
            .client
            .request(request.method, &request.url)
            .headers(request.headers);
        if let Some(body) = request.body {
            builder = builder.body(body);
        }

        let mut response = builder.send().await?;
        let status = response.status();
        let headers = response.headers().clone();

        // 非 2xx 不写文件;错误体通常很小,保留给上层判断。
        if !status.is_success() {
            let body = response.bytes().await?.to_vec();
            return Ok(DownloadResponse {
                status,
                headers,
                body,
                bytes: 0,
            });
        }

        let mut file = tokio::fs::File::create(dest).await?;
        let mut bytes = 0u64;
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if let Err(error) = file.write_all(&chunk).await {
                        drop(file);
                        let _ = tokio::fs::remove_file(dest).await;
                        return Err(error.into());
                    }
                    bytes += chunk.len() as u64;
                }
                Ok(None) => break,
                Err(error) => {
                    drop(file);
                    let _ = tokio::fs::remove_file(dest).await;
                    return Err(error.into());
                }
            }
        }

        if let Err(error) = file.flush().await {
            drop(file);
            let _ = tokio::fs::remove_file(dest).await;
            return Err(error.into());
        }

        Ok(DownloadResponse {
            status,
            headers,
            body: Vec::new(),
            bytes,
        })
    }
}
