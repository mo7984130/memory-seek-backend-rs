use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

use super::upload::{MINIMAL_JPEG, multipart_upload_request};

/// Test getting photos when no photos exist (empty list)
#[tokio::test]
async fn test_get_photos_empty() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "pget";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request("GET", "/photo?size=10", &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    assert!(
        json["data"]["records"].as_array().unwrap().is_empty(),
        "无照片时 records 应为空数组"
    );
    assert_eq!(json["data"]["hasMore"], false);

    guard.cleanup().await;
}

/// Test getting photos after uploading one
#[tokio::test]
async fn test_get_photos_after_upload() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "pga";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    // Upload a photo first
    let upload_req = multipart_upload_request("/photo", &user, MINIMAL_JPEG, "test.png");
    let upload_res = app.clone().oneshot(upload_req).await.unwrap();

    // If S3/MinIO is not available, skip this test
    if upload_res.status() == StatusCode::INTERNAL_SERVER_ERROR {
        guard.cleanup().await;
        return;
    }

    assert_eq!(upload_res.status(), StatusCode::OK);

    let upload_body = axum::body::to_bytes(upload_res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let upload_json: Value = serde_json::from_slice(&upload_body).unwrap();
    let photo_id = upload_json["data"]["id"].as_str().unwrap();

    // Get photos
    let req = auth::auth_request("GET", "/photo?size=10", &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    let records = json["data"]["records"].as_array().unwrap();
    assert_eq!(records.len(), 1, "应有 1 张照片");
    assert_eq!(records[0]["id"].as_str().unwrap(), photo_id);

    guard.cleanup().await;
}
