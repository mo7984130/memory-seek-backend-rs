//! HTTP 客户端:请求构造、鉴权注入、401 自动刷新与统一响应解析。

use std::sync::Arc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::config::ApiConfig;
use crate::error::{ApiError, Result};
use crate::storage::{AuthStorage, Storage};
use crate::transport::{
    HeaderMap, HeaderName, HeaderValue, HttpRequest, HttpResponse, Method, StatusCode, Transport,
};
use common_core::r::R;
use http::header;
use types_core::UserId;
use types_identity::auth::RefreshAccessTokenResponse;

const JSON_CONTENT_TYPE: &str = "application/json;charset=utf-8";

/// Memory Seek 后端客户端。
///
/// 通过构造函数注入 [`ApiConfig`]、[`Transport`] 与 [`Storage`],自身不绑定
/// 任何运行时或 HTTP 库。
pub struct Client {
    config: ApiConfig,
    transport: Arc<dyn Transport>,
    storage: Arc<dyn Storage>,
}

impl Client {
    /// 构造客户端。
    pub fn new(
        config: ApiConfig,
        transport: Arc<dyn Transport>,
        storage: Arc<dyn Storage>,
    ) -> Self {
        Self {
            config,
            transport,
            storage,
        }
    }

    /// 当前配置。
    pub fn config(&self) -> &ApiConfig {
        &self.config
    }

    /// 登录态存储操作器。
    pub fn auth_storage(&self) -> AuthStorage {
        AuthStorage::new(Arc::clone(&self.storage))
    }

    /// 凭 token 生成影像访问 URL(图片 / 视频封面 / 原视频流)。
    pub fn visual_url(&self, token: &str) -> String {
        format!("{}/visual/{token}", base(&self.config))
    }

    /// 发送请求并返回响应壳中的 `data`(JSON 值)。
    pub(crate) async fn send_value(&self, request: Request) -> Result<Value> {
        let response = self.execute(request).await?;
        parse_envelope(response.status, &response.body)
    }

    /// 免鉴权直发请求(不注入鉴权、不做 401 重试)。
    ///
    /// 仅内部用于刷新令牌:刷新请求本身若走 `execute` 会与鉴权流程形成
    /// 异步递归,故用此终端路径打断循环。
    async fn send_unauth(&self, request: Request) -> Result<Value> {
        let response = self
            .transport
            .send(self.build_http_request(&request, None))
            .await?;
        parse_envelope(response.status, &response.body)
    }

    /// 发送请求并把 `data` 反序列化为具体类型。
    pub(crate) async fn send_json<T: DeserializeOwned>(&self, request: Request) -> Result<T> {
        let value = self.send_value(request).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// 发送请求并忽略 `data`(用于无返回值的接口)。
    pub(crate) async fn send_unit(&self, request: Request) -> Result<()> {
        self.send_value(request).await?;
        Ok(())
    }

    /// 刷新 access token(不落盘);供 `api::auth::refresh_token` 复用。
    pub(crate) async fn fetch_refresh(
        &self,
        user_id: UserId,
        refresh_token: &str,
    ) -> Result<RefreshAccessTokenResponse> {
        let request = Request::post("/auth/token")
            .no_auth()
            .header("X-User-Id", user_id.to_string())
            .header("X-Refresh-Token", refresh_token.to_owned());
        let value = self.send_unauth(request).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// 执行请求:注入鉴权头;收到 401 时刷新一次并重试。
    async fn execute(&self, request: Request) -> Result<HttpResponse> {
        let auth = if request.auth {
            Some(self.auth_header().await?)
        } else {
            None
        };

        let response = self
            .transport
            .send(self.build_http_request(&request, auth.as_deref()))
            .await?;

        if response.status == StatusCode::UNAUTHORIZED && request.auth {
            self.refresh_and_store().await?;
            let auth = Some(self.auth_header().await?);
            return Ok(self
                .transport
                .send(self.build_http_request(&request, auth.as_deref()))
                .await?);
        }

        Ok(response)
    }

    /// 生成 `Authorization: Bearer {userId} {accessToken}`;token 缺失/过期时自动刷新。
    async fn auth_header(&self) -> Result<String> {
        let auth = self.auth_storage();
        let user_id = auth.user_id().ok_or(ApiError::NotLoggedIn)?;
        let access_token = match auth.access_token() {
            Some(token) => token,
            None => self.refresh_and_store().await?.access_token,
        };
        Ok(format!("Bearer {user_id} {access_token}"))
    }

    /// 使用存储中的 refresh token 刷新 access token,并写回存储。
    async fn refresh_and_store(&self) -> Result<RefreshAccessTokenResponse> {
        let auth = self.auth_storage();
        let user_id = auth.user_id().ok_or(ApiError::NotLoggedIn)?;
        let refresh_token = match auth.refresh_token() {
            Some(token) => token,
            None => {
                auth.clear_login_result();
                return Err(ApiError::TokenExpired);
            }
        };

        match self.fetch_refresh(user_id, &refresh_token).await {
            Ok(response) => {
                auth.set_access_token(&response.access_token, response.access_token_expire_at);
                Ok(response)
            }
            Err(error) => {
                auth.clear_login_result();
                Err(error)
            }
        }
    }

    /// 把内部请求描述转为传输层请求。
    fn build_http_request(&self, request: &Request, auth: Option<&str>) -> HttpRequest {
        let mut headers = HeaderMap::new();
        for (key, value) in &request.headers {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(key.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                headers.append(name, value);
            }
        }
        if let Some(content_type) = &request.content_type
            && let Ok(value) = HeaderValue::from_str(content_type)
        {
            headers.insert(header::CONTENT_TYPE, value);
        }
        if let Some(auth) = auth
            && let Ok(value) = HeaderValue::from_str(auth)
        {
            headers.insert(header::AUTHORIZATION, value);
        }

        HttpRequest {
            method: request.method.clone(),
            url: url_with_query(base(&self.config), &request.path, &request.query),
            headers,
            body: request.body.clone(),
        }
    }

    /// 低层请求入口:不解析响应包壳、不做 401 自动重试,直接返回原始 [`HttpResponse`]。
    ///
    /// 供测试或非 happy-path 调用方自控鉴权与观测状态码(如断言 401、读取二进制下载
    /// 内容与响应头、抓取非包壳响应)。常规业务调用请使用 [`api`](crate::api) 下的
    /// 类型化函数。
    ///
    /// 默认注入存储中的鉴权(未登录会报错);可经 [`RawRequest::no_auth`] 关闭,
    /// 或自行 [`RawRequest::header`] 设置 `Authorization`(此时不再注入)。
    pub fn raw(&self, method: Method, path: impl Into<String>) -> RawRequest<'_> {
        RawRequest {
            client: self,
            request: Request::new(method, path),
        }
    }
}

/// [`Client::raw`] 产出的低层请求构造器。
pub struct RawRequest<'a> {
    client: &'a Client,
    request: Request,
}

impl RawRequest<'_> {
    /// 标记免鉴权:不注入存储中的 `Authorization`。
    pub fn no_auth(mut self) -> Self {
        self.request = self.request.no_auth();
        self
    }

    /// 追加一个请求头(可用于显式指定 `Authorization`)。
    pub fn header(mut self, key: &str, value: impl Into<String>) -> Self {
        self.request = self.request.header(key, value);
        self
    }

    /// 追加一个查询参数(值经 serde 序列化为标量;`None`/`null` 自动省略)。
    pub fn query<T: Serialize>(mut self, key: &str, value: T) -> Self {
        self.request = self.request.query(key, value);
        self
    }

    /// 追加一个查询参数(值原样作为字符串)。
    pub fn query_raw(mut self, key: &str, value: impl Into<String>) -> Self {
        self.request = self.request.query_raw(key, value);
        self
    }

    /// 设置 JSON 请求体。
    pub fn json<T: Serialize>(mut self, body: &T) -> Self {
        self.request = self.request.json(body);
        self
    }

    /// 设置原始字节请求体(如文件上传)。
    pub fn bytes(mut self, body: Vec<u8>, content_type: Option<String>) -> Self {
        self.request = self.request.bytes(body, content_type);
        self
    }

    /// 发送一次并返回原始响应(不解析包壳、不重试)。
    ///
    /// 已显式设置 `Authorization` 头时不再注入存储中的鉴权。
    pub async fn send(self) -> Result<HttpResponse> {
        let has_auth_header = self
            .request
            .headers
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case("Authorization"));
        let auth = if self.request.auth && !has_auth_header {
            Some(self.client.auth_header().await?)
        } else {
            None
        };
        self.client
            .transport
            .send(
                self.client
                    .build_http_request(&self.request, auth.as_deref()),
            )
            .await
            .map_err(ApiError::from)
    }
}

/// 解析响应:以 **HTTP 状态码** 为权威判定成功/失败,包壳 `R` 仅用于取 `data`/`msg`。
///
/// - 2xx:解析包壳并返回其 `data`(缺省为 `null`);
/// - 非 2xx:返回 [`ApiError::HttpStatus`],`code` 为 HTTP 状态码;`msg` 尽量取自
///   包壳 `msg`,取不到(如中间件/网关直接返回的非包壳响应)则退化为响应体原文,
///   绝不因解析失败而丢掉状态码。
fn parse_envelope(status: StatusCode, body: &[u8]) -> Result<Value> {
    if !status.is_success() {
        let msg = serde_json::from_slice::<R<Value>>(body)
            .ok()
            .and_then(|envelope| envelope.msg)
            .unwrap_or_else(|| {
                let text = String::from_utf8_lossy(body);
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    "未知错误".to_owned()
                } else {
                    trimmed.to_owned()
                }
            });
        return Err(ApiError::HttpStatus {
            code: status.as_u16(),
            msg,
        });
    }
    let envelope: R<Value> = serde_json::from_slice(body)?;
    Ok(envelope.data.unwrap_or(Value::Null))
}

/// 裁剪末尾斜杠后的基础 URL。
fn base(config: &ApiConfig) -> &str {
    config.base_url.trim_end_matches('/')
}

/// 拼接路径与查询串。
fn url_with_query(base: &str, path: &str, query: &[(String, String)]) -> String {
    let mut url = format!("{base}{path}");
    if !query.is_empty() {
        let encoded: Vec<String> = query
            .iter()
            .map(|(key, value)| format!("{}={}", encode_component(key), encode_component(value)))
            .collect();
        url.push('?');
        url.push_str(&encoded.join("&"));
    }
    url
}

/// percent-encode 查询串部件(保留非保留字符,其余按 UTF-8 字节编码)。
fn encode_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for &byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// 把一个 JSON 值转为查询串标量;`null` 返回 `None`(表示省略该参数)。
fn scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        other => Some(other.to_string()),
    }
}

/// 内部请求构造器。
pub(crate) struct Request {
    method: Method,
    path: String,
    query: Vec<(String, String)>,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    content_type: Option<String>,
    auth: bool,
}

impl Request {
    fn new(method: Method, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            query: Vec::new(),
            headers: Vec::new(),
            body: None,
            content_type: None,
            auth: true,
        }
    }

    /// GET 请求。
    pub(crate) fn get(path: impl Into<String>) -> Self {
        Self::new(Method::GET, path)
    }

    /// POST 请求。
    pub(crate) fn post(path: impl Into<String>) -> Self {
        Self::new(Method::POST, path)
    }

    /// PUT 请求。
    pub(crate) fn put(path: impl Into<String>) -> Self {
        Self::new(Method::PUT, path)
    }

    /// PATCH 请求。
    pub(crate) fn patch(path: impl Into<String>) -> Self {
        Self::new(Method::PATCH, path)
    }

    /// DELETE 请求。
    pub(crate) fn delete(path: impl Into<String>) -> Self {
        Self::new(Method::DELETE, path)
    }

    /// 标记为免鉴权请求(如 `/auth/*`)。
    pub(crate) fn no_auth(mut self) -> Self {
        self.auth = false;
        self
    }

    /// 追加一个请求头。
    pub(crate) fn header(mut self, key: &str, value: impl Into<String>) -> Self {
        self.headers.push((key.to_owned(), value.into()));
        self
    }

    /// 追加一个查询参数(值经 serde 序列化为标量;`None`/`null` 自动省略)。
    pub(crate) fn query<T: Serialize>(mut self, key: &str, value: T) -> Self {
        if let Ok(value) = serde_json::to_value(&value)
            && let Some(scalar) = scalar_to_string(&value)
        {
            self.query.push((key.to_owned(), scalar));
        }
        self
    }

    /// 追加一个查询参数(值原样作为字符串)。
    pub(crate) fn query_raw(mut self, key: &str, value: impl Into<String>) -> Self {
        self.query.push((key.to_owned(), value.into()));
        self
    }

    /// 设置 JSON 请求体。
    pub(crate) fn json<T: Serialize>(mut self, body: &T) -> Self {
        self.body = Some(serde_json::to_vec(body).expect("请求体必须可序列化"));
        self.content_type = Some(JSON_CONTENT_TYPE.to_owned());
        self
    }

    /// 设置原始字节请求体(如文件上传)。
    pub(crate) fn bytes(mut self, body: Vec<u8>, content_type: Option<String>) -> Self {
        self.body = Some(body);
        self.content_type =
            Some(content_type.unwrap_or_else(|| "application/octet-stream".to_owned()));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryStorage;
    use crate::transport::TransportError;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// 记录所有请求并按序返回预设响应的测试用传输层。
    struct MockTransport {
        requests: Mutex<Vec<HttpRequest>>,
        responses: Mutex<VecDeque<HttpResponse>>,
    }

    impl MockTransport {
        fn queue(responses: Vec<HttpResponse>) -> Arc<Self> {
            Arc::new(Self {
                requests: Mutex::new(Vec::new()),
                responses: Mutex::new(responses.into()),
            })
        }

        fn requests(&self) -> Vec<HttpRequest> {
            self.requests.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl Transport for MockTransport {
        async fn send(
            &self,
            request: HttpRequest,
        ) -> std::result::Result<HttpResponse, TransportError> {
            self.requests.lock().unwrap().push(request);
            Ok(self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("测试未提供足够的预设响应"))
        }
    }

    fn ok_response(data: &str) -> HttpResponse {
        HttpResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: format!(r#"{{"code":200,"data":{data}}}"#).into_bytes(),
        }
    }

    fn header<'a>(request: &'a HttpRequest, key: &str) -> Option<&'a str> {
        request
            .headers
            .get(key)
            .and_then(|value| value.to_str().ok())
    }

    /// 构造一个 access / refresh token 均远未过期的登录结果。
    fn login_response() -> crate::LoginResponse {
        let far_future = crate::DateTime::from_timestamp(4_102_444_800, 0).unwrap();
        crate::LoginResponse {
            user: crate::UserInfo {
                id: crate::UserId(1),
                username: "alice".to_owned(),
                nickname: "Alice".to_owned(),
                email: "alice@example.com".to_owned(),
                avatar_token: None,
                created_at: far_future,
            },
            access_token: "old-token".to_owned(),
            access_token_expire_at: far_future,
            refresh_token: "refresh-token".to_owned(),
            refresh_token_expire_at: far_future,
        }
    }

    #[test]
    fn query_encoding_skips_null_and_encodes() {
        let request = Request::get("/visual")
            .query("size", Some(20u64))
            .query("skip", Option::<u64>::None)
            .query("anchorTime", "2024-01-02T03:04:05Z");
        let url = url_with_query("/api", &request.path, &request.query);
        assert_eq!(
            url,
            "/api/visual?size=20&anchorTime=2024-01-02T03%3A04%3A05Z"
        );
    }

    #[test]
    fn encode_component_percent_encodes_reserved() {
        assert_eq!(encode_component("a/b c"), "a%2Fb%20c");
    }

    #[test]
    fn login_request_has_no_auth_header() {
        let transport = MockTransport::queue(vec![ok_response("{}")]);
        let client = Client::new(
            ApiConfig::new("/api"),
            transport.clone(),
            Arc::new(MemoryStorage::new()),
        );
        let result =
            futures::executor::block_on(client.send_value(Request::post("/auth/login").no_auth()));
        assert!(result.is_ok());

        let requests = transport.requests();
        assert_eq!(requests[0].method, Method::POST);
        assert!(header(&requests[0], "Authorization").is_none());
    }

    #[test]
    fn authenticated_request_without_login_errors() {
        let transport = MockTransport::queue(vec![]);
        let client = Client::new(
            ApiConfig::new("/api"),
            transport.clone(),
            Arc::new(MemoryStorage::new()),
        );
        let result = futures::executor::block_on(client.send_value(Request::get("/user/me")));
        assert!(matches!(result, Err(ApiError::NotLoggedIn)));
        assert!(transport.requests().is_empty());
    }

    #[test]
    fn http_status_error_is_surfaced_with_envelope_msg() {
        // 非 2xx:以 HTTP 状态码为权威,msg 取自包壳。
        let transport = MockTransport::queue(vec![HttpResponse {
            status: StatusCode::BAD_REQUEST,
            headers: HeaderMap::new(),
            body: br#"{"code":400,"msg":"bad"}"#.to_vec(),
        }]);
        let client = Client::new(
            ApiConfig::new("/api"),
            transport.clone(),
            Arc::new(MemoryStorage::new()),
        );
        let result = futures::executor::block_on(client.send_value(Request::get("/x").no_auth()));
        match result {
            Err(ApiError::HttpStatus { code, msg }) => {
                assert_eq!(code, 400);
                assert_eq!(msg, "bad");
            }
            other => panic!("expected http status error, got {other:?}"),
        }
    }

    #[test]
    fn http_status_error_survives_non_envelope_body() {
        // 非 2xx 且 body 不是包壳(如中间件/网关直返):仍按 HTTP 状态码上报,
        // 不退化为 Decode,msg 取响应体原文。
        let transport = MockTransport::queue(vec![HttpResponse {
            status: StatusCode::SERVICE_UNAVAILABLE,
            headers: HeaderMap::new(),
            body: b"upload busy".to_vec(),
        }]);
        let client = Client::new(
            ApiConfig::new("/api"),
            transport.clone(),
            Arc::new(MemoryStorage::new()),
        );
        let result = futures::executor::block_on(client.send_value(Request::get("/x").no_auth()));
        match result {
            Err(ApiError::HttpStatus { code, msg }) => {
                assert_eq!(code, 503);
                assert_eq!(msg, "upload busy");
            }
            other => panic!("expected http status error, got {other:?}"),
        }
    }

    #[test]
    fn raw_send_skips_envelope_and_returns_status() {
        // 4xx + 非包壳 body:raw 直接回原始响应,不做包壳解析、不重试
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/plain"),
        );
        let transport = MockTransport::queue(vec![HttpResponse {
            status: StatusCode::UNAUTHORIZED,
            headers,
            body: b"unauthorized".to_vec(),
        }]);
        let client = Client::new(
            ApiConfig::new("/api"),
            transport.clone(),
            Arc::new(MemoryStorage::new()),
        );
        let response =
            futures::executor::block_on(client.raw(Method::GET, "/user/me").no_auth().send())
                .unwrap();
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
        assert_eq!(response.body, b"unauthorized".to_vec());
        assert_eq!(
            response
                .headers
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("text/plain")
        );
        let requests = transport.requests();
        assert!(header(&requests[0], "Authorization").is_none());
    }

    #[test]
    fn raw_send_uses_explicit_authorization_only() {
        let transport = MockTransport::queue(vec![HttpResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Vec::new(),
        }]);
        let storage = Arc::new(MemoryStorage::new());
        AuthStorage::new(storage.clone()).set_login_result(&login_response());
        let client = Client::new(ApiConfig::new("/api"), transport.clone(), storage);
        let response = futures::executor::block_on(
            client
                .raw(Method::GET, "/user/me")
                .header("Authorization", "Bearer 1 stale-token")
                .send(),
        )
        .unwrap();
        assert_eq!(response.status, StatusCode::OK);
        // 显式头生效,且不与存储自动注入叠加
        let requests = transport.requests();
        let auth_headers: Vec<&str> = requests[0]
            .headers
            .get_all("authorization")
            .iter()
            .filter_map(|value| value.to_str().ok())
            .collect();
        assert_eq!(auth_headers, vec!["Bearer 1 stale-token"]);
    }

    #[test]
    fn refreshes_access_token_on_401_and_retries() {
        let transport = MockTransport::queue(vec![
            // 首次业务请求:401
            HttpResponse {
                status: StatusCode::UNAUTHORIZED,
                headers: HeaderMap::new(),
                body: br#"{"code":401,"msg":"expired"}"#.to_vec(),
            },
            // 刷新令牌请求:成功,返回新 access token
            ok_response(
                r#"{"accessToken":"new-token","accessTokenExpireAt":"2100-01-01T00:00:00Z"}"#,
            ),
            // 重试业务请求:成功
            ok_response(r#"{"ok":true}"#),
        ]);
        let storage = Arc::new(MemoryStorage::new());
        AuthStorage::new(storage.clone()).set_login_result(&login_response());

        let client = Client::new(ApiConfig::new("/api"), transport.clone(), storage);
        let result = futures::executor::block_on(
            client.send_json::<serde_json::Value>(Request::get("/user/me")),
        );
        assert_eq!(result.unwrap()["ok"], serde_json::json!(true));

        let requests = transport.requests();
        assert_eq!(requests.len(), 3);
        // 首次请求携带旧 token
        assert_eq!(
            header(&requests[0], "Authorization"),
            Some("Bearer 1 old-token")
        );
        // 刷新请求携带 refresh token
        assert_eq!(
            header(&requests[1], "X-Refresh-Token"),
            Some("refresh-token")
        );
        assert!(header(&requests[1], "Authorization").is_none());
        // 重试请求携带新 token
        assert_eq!(
            header(&requests[2], "Authorization"),
            Some("Bearer 1 new-token")
        );
    }
    #[test]
    fn deserializes_avatar_token_without_cipher() {
        // 关键回归:UserInfo 的 avatar_token(VisualTokenStr)无需密钥也能反序列化
        let json = br#"{
            "id": "1",
            "username": "alice",
            "nickname": "Alice",
            "email": "alice@example.com",
            "avatarToken": "opaque-encrypted-token",
            "createdAt": "2024-01-02T03:04:05Z"
        }"#;
        let user: crate::UserInfo = serde_json::from_slice(json).unwrap();
        assert_eq!(user.nickname, "Alice");
        assert_eq!(
            user.avatar_token.as_ref().unwrap().as_str(),
            "opaque-encrypted-token"
        );
    }
}
