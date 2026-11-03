//! 集成测试共用的 Mock 传输层与请求断言工具。

#![allow(dead_code)]

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use memory_seek_api::{
    ApiConfig, AuthStorage, Client, DateTime, DownloadResponse, HeaderMap, HeaderName, HeaderValue,
    HttpRequest, HttpResponse, MemoryStorage, StatusCode, Transport, TransportError, UserId,
};
use serde_json::{Value, json};

/// 记录所有请求并按序返回预设响应的测试传输层。
pub struct MockTransport {
    requests: Mutex<Vec<HttpRequest>>,
    responses: Mutex<VecDeque<HttpResponse>>,
}

impl MockTransport {
    pub fn new(responses: Vec<HttpResponse>) -> Arc<Self> {
        Arc::new(Self {
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(responses.into()),
        })
    }

    pub fn requests(&self) -> Vec<HttpRequest> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl Transport for MockTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.requests.lock().unwrap().push(request);
        Ok(self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("测试未提供足够的预设响应"))
    }

    async fn download_to(
        &self,
        request: HttpRequest,
        dest: &Path,
    ) -> Result<DownloadResponse, TransportError> {
        let response = self.send(request).await?;
        let bytes = response.body.len() as u64;
        if response.status.is_success() {
            std::fs::write(dest, &response.body)?;
            Ok(DownloadResponse {
                status: response.status,
                headers: response.headers,
                body: Vec::new(),
                bytes,
            })
        } else {
            Ok(DownloadResponse {
                status: response.status,
                headers: response.headers,
                body: response.body,
                bytes: 0,
            })
        }
    }
}

/// 构造一个已登录的客户端与对应的 Mock 传输层。
pub fn client(responses: Vec<HttpResponse>) -> (Client, Arc<MockTransport>) {
    let transport = MockTransport::new(responses);
    let storage = Arc::new(MemoryStorage::new());
    let auth = AuthStorage::new(storage.clone());
    auth.set_user_id(UserId(7)).unwrap();
    let expire_at = DateTime::from_timestamp(4_102_444_800, 0).unwrap();
    auth.set_access_token("access", expire_at).unwrap();
    auth.set_refresh_token("refresh", expire_at).unwrap();
    (
        Client::new(ApiConfig::new("/api"), transport.clone(), storage),
        transport,
    )
}

/// 构造一个 `R` 包壳的成功响应。
pub fn ok(data: Value) -> HttpResponse {
    HttpResponse {
        status: StatusCode::OK,
        headers: HeaderMap::new(),
        body: serde_json::to_vec(&json!({ "code": 200, "data": data })).unwrap(),
    }
}

/// 构造一个 `text/plain` 的成功响应。
pub fn ok_text(body: &str) -> HttpResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("content-type"),
        HeaderValue::from_static("text/plain"),
    );
    HttpResponse {
        status: StatusCode::OK,
        headers,
        body: body.as_bytes().to_vec(),
    }
}

pub fn header<'a>(request: &'a HttpRequest, key: &str) -> Option<&'a str> {
    request
        .headers
        .get(key)
        .and_then(|value| value.to_str().ok())
}

pub fn assert_authorized(request: &HttpRequest) {
    assert_eq!(header(request, "authorization"), Some("Bearer 7 access"));
}

pub fn body_json(request: &HttpRequest) -> Value {
    serde_json::from_slice(request.body.as_deref().expect("请求应包含 body")).unwrap()
}

pub fn empty_page() -> Value {
    json!({ "records": [], "nextCursor": null, "hasMore": false })
}

pub fn empty_page_with_total() -> Value {
    json!({
        "page": { "records": [], "nextCursor": null, "hasMore": false },
        "totalCount": 0
    })
}

pub fn admin_job(kind: &str) -> HttpResponse {
    ok(json!({
        "id": "job-1",
        "kind": kind,
        "status": "queued",
        "attempts": 0,
        "requestedBy": "1",
        "createdAt": "2026-01-01T00:00:00Z",
        "startedAt": null,
        "finishedAt": null,
        "result": null,
        "error": null
    }))
}
