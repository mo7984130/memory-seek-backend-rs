//! 新增接口的请求构造 / 响应解析测试。

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use memory_seek_api::{
    ApiConfig, AuthStorage, BackupSource, BackupTier, Client, CountIdCursor, DateTime,
    DownloadResponse, FaceId, FaceIds, HeaderMap, HeaderName, HeaderValue, HttpRequest,
    HttpResponse, MemoryStorage, Method, PersonId, PersonName, RestoreRequest, StatusCode,
    TimeIdCursor, Transport, TransportError, UserId, VisualId,
};
use serde_json::{Value, json};

/// 记录所有请求并按序返回预设响应的测试传输层。
struct MockTransport {
    requests: Mutex<Vec<HttpRequest>>,
    responses: Mutex<VecDeque<HttpResponse>>,
}

impl MockTransport {
    fn new(responses: Vec<HttpResponse>) -> Arc<Self> {
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

fn client(responses: Vec<HttpResponse>) -> (Client, Arc<MockTransport>) {
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

fn ok(data: Value) -> HttpResponse {
    HttpResponse {
        status: StatusCode::OK,
        headers: HeaderMap::new(),
        body: serde_json::to_vec(&json!({ "code": 200, "data": data })).unwrap(),
    }
}

fn ok_text(body: &str) -> HttpResponse {
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

fn header<'a>(request: &'a HttpRequest, key: &str) -> Option<&'a str> {
    request
        .headers
        .get(key)
        .and_then(|value| value.to_str().ok())
}

fn assert_authorized(request: &HttpRequest) {
    assert_eq!(header(request, "authorization"), Some("Bearer 7 access"));
}

fn body_json(request: &HttpRequest) -> Value {
    serde_json::from_slice(request.body.as_deref().expect("请求应包含 body")).unwrap()
}

fn empty_page() -> Value {
    json!({ "records": [], "nextCursor": null, "hasMore": false })
}

#[test]
fn face_queries_build_expected_requests() {
    let (client, transport) = client(vec![
        ok(json!([])),
        ok(empty_page()),
        ok(json!(null)),
        ok(json!(null)),
        ok(json!(null)),
        ok(json!({ "deletedFaceCount": 2 })),
    ]);

    let faces = block_on(memory_seek_api::api::visual::face::get_faces(
        &client,
        VisualId(1),
    ))
    .unwrap();
    assert!(faces.is_empty());

    let cursor = TimeIdCursor {
        time_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        id: VisualId(9),
    };
    let page = block_on(
        memory_seek_api::api::visual::face::get_unassigned_face_visuals(
            &client,
            Some(cursor.clone()),
            Some(10),
        ),
    )
    .unwrap();
    assert!(page.records.is_empty());

    block_on(memory_seek_api::api::visual::face::change_face_belonging(
        &client,
        FaceId(2),
        Some(PersonId(3)),
    ))
    .unwrap();
    block_on(memory_seek_api::api::visual::face::change_face_belonging(
        &client,
        FaceId(2),
        None,
    ))
    .unwrap();
    block_on(memory_seek_api::api::visual::face::delete_face(
        &client,
        FaceId(4),
    ))
    .unwrap();

    let deleted = block_on(memory_seek_api::api::visual::face::delete_faces_batch(
        &client,
        FaceIds::new(vec![FaceId(5), FaceId(6)]).unwrap(),
    ))
    .unwrap();
    assert_eq!(deleted.deleted_face_count, 2);

    let requests = transport.requests();
    assert_eq!(requests.len(), 6);

    assert_eq!(requests[0].method, Method::GET);
    assert_eq!(requests[0].url, "/api/visual/face/visual/1");
    assert_authorized(&requests[0]);

    assert_eq!(requests[1].method, Method::GET);
    assert!(
        requests[1]
            .url
            .starts_with("/api/visual/face/unassigned-visuals?")
    );
    assert!(
        requests[1]
            .url
            .contains(&format!("cursor={}", cursor.encode()))
    );
    assert!(requests[1].url.contains("size=10"));

    assert_eq!(requests[2].method, Method::POST);
    assert_eq!(requests[2].url, "/api/visual/face/feature/2/belonging/3");
    assert_eq!(requests[2].body, None);

    assert_eq!(requests[3].method, Method::POST);
    assert_eq!(requests[3].url, "/api/visual/face/feature/2/belonging");

    assert_eq!(requests[4].method, Method::DELETE);
    assert_eq!(requests[4].url, "/api/visual/face/feature/4");

    assert_eq!(requests[5].method, Method::DELETE);
    assert_eq!(requests[5].url, "/api/visual/face/feature");
    assert_eq!(body_json(&requests[5]), json!({ "faceIds": ["5", "6"] }));
}

#[test]
fn face_admin_endpoints_build_expected_requests() {
    let (client, transport) = client(vec![ok(json!(null)), ok(json!(null))]);

    block_on(memory_seek_api::api::visual::face::full_compute(&client)).unwrap();
    block_on(memory_seek_api::api::visual::face::incremental_compute(
        &client,
    ))
    .unwrap();

    let requests = transport.requests();
    assert_eq!(requests[0].method, Method::GET);
    assert_eq!(requests[0].url, "/api/visual/face/admin/full");
    assert_eq!(requests[1].method, Method::GET);
    assert_eq!(requests[1].url, "/api/visual/face/admin/incremental");
}

#[test]
fn person_endpoints_build_expected_requests() {
    let (client, transport) = client(vec![
        ok(empty_page()),
        ok(empty_page()),
        ok(empty_page()),
        ok(json!(null)),
        ok(json!(null)),
        ok(json!({
            "id": "10",
            "name": "Alice",
            "coverToken": "cover",
            "faceCount": 3
        })),
        ok(json!(null)),
        ok(json!(null)),
    ]);

    let cursor = CountIdCursor {
        count: 5,
        id: PersonId(3),
    };
    let persons = block_on(memory_seek_api::api::visual::person::get_persons(
        &client,
        Some(cursor.clone()),
        Some(20),
    ))
    .unwrap();
    assert!(persons.records.is_empty());

    let page = block_on(memory_seek_api::api::visual::person::search_persons(
        &client,
        "ZS",
        Some(PersonId(9)),
        None,
    ))
    .unwrap();
    assert!(page.records.is_empty());

    let page = block_on(memory_seek_api::api::visual::person::get_person_visuals(
        &client,
        PersonId(4),
        None,
        Some(50),
    ))
    .unwrap();
    assert!(page.records.is_empty());

    block_on(memory_seek_api::api::visual::person::rename_person(
        &client,
        PersonId(4),
        PersonName::new("张三".to_owned()).unwrap(),
    ))
    .unwrap();

    block_on(memory_seek_api::api::visual::person::delete_person(
        &client,
        PersonId(4),
    ))
    .unwrap();

    let merged = block_on(memory_seek_api::api::visual::person::merge_person(
        &client,
        PersonId(1),
        PersonId(2),
    ))
    .unwrap();
    assert_eq!(merged.id, PersonId(10));
    assert_eq!(merged.name, "Alice");

    block_on(memory_seek_api::api::visual::person::full_scan(&client)).unwrap();
    block_on(memory_seek_api::api::visual::person::secondary_cluster(
        &client,
        &memory_seek_api::SecondaryClusterParam { threshold: 0.6 },
    ))
    .unwrap();

    let requests = transport.requests();
    assert_eq!(requests.len(), 8);

    assert_eq!(requests[0].method, Method::GET);
    assert!(requests[0].url.starts_with("/api/visual/person?"));
    assert!(
        requests[0]
            .url
            .contains(&format!("cursor={}", cursor.encode()))
    );
    assert!(requests[0].url.contains("size=20"));

    assert_eq!(requests[1].method, Method::GET);
    assert!(requests[1].url.starts_with("/api/visual/person/search?"));
    assert!(requests[1].url.contains("keyword=ZS"));
    assert!(requests[1].url.contains("cursor=9"));
    assert!(requests[1].url.contains("size=32"));

    assert_eq!(requests[2].method, Method::GET);
    assert_eq!(requests[2].url, "/api/visual/person/4/visuals?size=50");

    assert_eq!(requests[3].method, Method::POST);
    assert_eq!(requests[3].url, "/api/visual/person/4/name");
    assert_eq!(body_json(&requests[3]), json!({ "newName": "张三" }));

    assert_eq!(requests[4].method, Method::DELETE);
    assert_eq!(requests[4].url, "/api/visual/person/4");

    assert_eq!(requests[5].method, Method::POST);
    assert_eq!(requests[5].url, "/api/visual/person/merge");
    assert_eq!(
        body_json(&requests[5]),
        json!({ "sourcePersonId": "1", "targetPersonId": "2" })
    );

    assert_eq!(requests[6].method, Method::GET);
    assert_eq!(requests[6].url, "/api/visual/person/admin/full_scan");

    assert_eq!(requests[7].method, Method::POST);
    assert_eq!(
        requests[7].url,
        "/api/visual/person/admin/secondary_cluster"
    );
    assert_eq!(body_json(&requests[7]), json!({ "threshold": 0.6 }));
}

#[test]
fn backup_endpoints_build_expected_requests() {
    let (client, transport) = client(vec![
        ok(json!({
            "exported": 4,
            "failed": 0,
            "cleaned": 2,
            "durationSecs": 1.5
        })),
        ok(json!({ "exported": 3, "failed": 1, "durationSecs": 0.5 })),
        ok(json!({ "restored": 42 })),
    ]);

    let scheduled = block_on(memory_seek_api::api::backup::trigger_backup(&client)).unwrap();
    assert_eq!(scheduled.exported, 4);
    assert_eq!(scheduled.cleaned, 2);

    let manual = block_on(memory_seek_api::api::backup::manual_backup(&client)).unwrap();
    assert_eq!(manual.failed, 1);

    let request = RestoreRequest {
        tier: BackupTier::Daily,
        source: BackupSource::Local,
        run_id: "run-1".to_owned(),
        confirm_run_id: "run-1".to_owned(),
    };
    let restored = block_on(memory_seek_api::api::backup::restore_backup(
        &client, &request,
    ))
    .unwrap();
    assert_eq!(restored.restored, 42);

    let requests = transport.requests();
    assert_eq!(requests[0].method, Method::POST);
    assert_eq!(requests[0].url, "/api/admin/backup/trigger");
    assert_eq!(requests[1].url, "/api/admin/backup/manual");
    assert_eq!(requests[2].url, "/api/admin/backup/restore");
    assert_eq!(
        body_json(&requests[2]),
        json!({
            "tier": "daily",
            "source": "local",
            "runId": "run-1",
            "confirmRunId": "run-1"
        })
    );
}

#[test]
fn health_returns_plain_text_without_auth() {
    let (client, transport) = client(vec![ok_text("ok")]);
    let health = block_on(memory_seek_api::api::system::health(&client)).unwrap();
    assert_eq!(health, "ok");

    let requests = transport.requests();
    assert_eq!(requests[0].method, Method::GET);
    assert_eq!(requests[0].url, "/api/health");
    assert!(header(&requests[0], "authorization").is_none());
}

#[test]
fn download_range_sends_header_and_reports_ready() {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("content-range"),
        HeaderValue::from_static("bytes 0-2/10"),
    );
    let (client, transport) = client(vec![HttpResponse {
        status: StatusCode::PARTIAL_CONTENT,
        headers,
        body: b"abc".to_vec(),
    }]);

    let response = block_on(memory_seek_api::api::visual::download_range(
        &client,
        "token",
        "bytes=0-2",
    ))
    .unwrap();
    assert!(response.is_ready());
    assert_eq!(response.status, StatusCode::PARTIAL_CONTENT);

    let requests = transport.requests();
    assert_eq!(requests[0].url, "/api/visual/token");
    assert_eq!(header(&requests[0], "range"), Some("bytes=0-2"));
}

#[test]
fn download_to_cleans_up_not_ready_file() {
    let (client, _transport) = client(vec![HttpResponse {
        status: StatusCode::ACCEPTED,
        headers: HeaderMap::new(),
        body: Vec::new(),
    }]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("video.mp4");

    let response = block_on(memory_seek_api::api::visual::download_to(
        &client, "token", &path,
    ))
    .unwrap();
    assert_eq!(response.status, StatusCode::ACCEPTED);
    assert!(!response.is_ready());
    assert!(!path.exists());
}

#[test]
fn raw_download_response_ready_helpers() {
    let response = HttpResponse {
        status: StatusCode::ACCEPTED,
        headers: HeaderMap::new(),
        body: Vec::new(),
    };
    assert!(response.is_success());
    assert!(!response.is_ready());

    let response = HttpResponse {
        status: StatusCode::OK,
        headers: HeaderMap::new(),
        body: Vec::new(),
    };
    assert!(response.is_ready());
}
