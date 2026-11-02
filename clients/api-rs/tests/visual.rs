//! 影像下载相关接口的测试。

mod common;

use futures::executor::block_on;
use memory_seek_api::{HeaderMap, HeaderName, HeaderValue, HttpResponse, StatusCode};

use common::{client, header};

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
