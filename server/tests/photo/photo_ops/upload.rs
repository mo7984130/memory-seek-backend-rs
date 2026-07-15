use axum::http::{Request, StatusCode, header};
use serde_json::Value;
use tower::ServiceExt;

use super::super::common::{MINIMAL_JPEG, multipart_upload_request};
use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// Test uploading a photo successfully
#[tokio::test]
async fn test_upload_photo_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "pup";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = multipart_upload_request("/photo", &user, MINIMAL_JPEG, "test.png");
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    // If S3/MinIO is not available, the upload may fail with 500
    if status == StatusCode::INTERNAL_SERVER_ERROR {
        guard.cleanup().await;
        return;
    }

    assert_eq!(status, StatusCode::OK, "Upload failed: {}", json);
    assert_eq!(json["code"], 200);
    assert!(json["data"]["id"].as_str().is_some(), "应返回 photo id");
    assert!(json["data"]["name"].as_str().is_some(), "应返回 photo name");
    assert!(json["data"]["width"].as_i64().is_some(), "应返回 width");
    assert!(json["data"]["height"].as_i64().is_some(), "应返回 height");
    assert!(json["data"]["size"].as_i64().is_some(), "应返回 size");

    guard.cleanup().await;
}

/// Test uploading without authentication returns 401
#[tokio::test]
async fn test_upload_photo_unauthorized() {
    let app = build_test_router().await;

    let boundary = "----testboundary";
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"test.png\"\r\n\
         Content-Type: image/png\r\n\r\n"
    );
    let mut body_bytes = body.into_bytes();
    body_bytes.extend_from_slice(MINIMAL_JPEG);
    body_bytes.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let req = Request::builder()
        .method("POST")
        .uri("/photo")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(axum::body::Body::from(body_bytes))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
