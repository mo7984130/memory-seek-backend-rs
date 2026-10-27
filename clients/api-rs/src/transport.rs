//! 传输层抽象:与具体 HTTP 库(以及运行时)解耦。
//!
//! 本 crate 只负责**构造**请求与**解析**响应;真正的发送由调用方注入的
//! [`Transport`] 实现完成。请求/响应描述复用 [`http`] crate 的标准类型
//! ([`Method`] / [`HeaderMap`] / [`StatusCode`])——`http` 仅提供类型, 不含客户端
//! 或运行时, 故不破坏"运行时/HTTP 库无关"的定位, 且与 reqwest / hyper 等零成本互转。

pub use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};

use std::path::Path;
use std::sync::Arc;

#[cfg(feature = "reqwest-transport")]
mod reqwest;

#[cfg(feature = "reqwest-transport")]
pub use self::reqwest::ReqwestTransport;

/// 与 HTTP 库无关的请求描述。
///
/// `url` 为**完整** URL(已包含 `base_url`、路径与查询串);请求体为原始字节,
/// 由 [`headers`](Self::headers) 中的 `Content-Type` 描述其语义。
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// 请求方法。
    pub method: Method,
    /// 完整请求 URL。
    pub url: String,
    /// 请求头。
    pub headers: HeaderMap,
    /// 请求体字节;`None` 表示无请求体。
    pub body: Option<Vec<u8>>,
}

/// 与 HTTP 库无关的响应描述。
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP 状态码。
    pub status: StatusCode,
    /// 响应头。供调用方读取 `Content-Type` 等。
    pub headers: HeaderMap,
    /// 响应体字节。
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// 状态码是否成功(2xx)。
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }
}

/// 流式下载的结果。
///
/// 成功(2xx)时响应体已经写入目标文件,`body` 为空;非 2xx 时不创建/保留目标文件,
/// 错误响应体放在 `body` 里供调用方判断。
#[derive(Debug, Clone)]
pub struct DownloadResponse {
    /// HTTP 状态码。
    pub status: StatusCode,
    /// 响应头(如 `Content-Type`、`Content-Length`)。
    pub headers: HeaderMap,
    /// 非 2xx 时的错误响应体;成功时为空。
    pub body: Vec<u8>,
    /// 已写入目标文件的字节数;非 2xx 时为 0。
    pub bytes: u64,
}

impl DownloadResponse {
    /// 状态码是否成功(2xx)。
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }
}

/// 传输层错误:由调用方实现自行决定错误类型。
pub type TransportError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// 传输层抽象:由调用方注入真实 HTTP 实现。
///
/// 实现方只负责发送 [`HttpRequest`] 并返回 [`HttpResponse`],不应做业务逻辑
/// (如 token 刷新由 [`Client`](crate::Client) 统一处理)。
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// 发送一个请求并返回响应。
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError>;

    /// 发送请求并把响应体写入 `dest`。
    ///
    /// 这是一个必选方法:实现方必须明确选择流式写入还是整段缓冲,不提供会偷偷
    /// 把整个响应读进内存的默认实现。非流式实现可以在内部复用 [`send`](Self::send)
    /// 后自行写文件。
    async fn download(
        &self,
        request: HttpRequest,
        dest: &Path,
    ) -> Result<DownloadResponse, TransportError>;
}

/// 让共享句柄也能作为 [`Transport`] 注入(如 `Arc<MyTransport>` / `Arc<dyn Transport>`)。
#[async_trait::async_trait]
impl<T: Transport + ?Sized> Transport for Arc<T> {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        (**self).send(request).await
    }

    async fn download(
        &self,
        request: HttpRequest,
        dest: &Path,
    ) -> Result<DownloadResponse, TransportError> {
        (**self).download(request, dest).await
    }
}

/// 让装箱实现也能作为 [`Transport`] 注入(如 `Box<dyn Transport>`)。
#[async_trait::async_trait]
impl<T: Transport + ?Sized> Transport for Box<T> {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        (**self).send(request).await
    }

    async fn download(
        &self,
        request: HttpRequest,
        dest: &Path,
    ) -> Result<DownloadResponse, TransportError> {
        (**self).download(request, dest).await
    }
}
