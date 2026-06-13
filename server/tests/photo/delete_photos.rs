use axum::http::StatusCode;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

use super::upload::{MINIMAL_JPEG, multipart_upload_request};

/// Test deleting photos successfully
#[tokio::test]
async fn test_delete_photos_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "pdel";
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
    let photo_id = upload_json["data"]["id"].as_str().unwrap().to_string();

    // Delete the photo
    let req = auth::auth_request(
        "DELETE",
        "/photo",
        &user,
        json!({ "photoIds": [photo_id] }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["code"], 200);

    guard.cleanup().await;
}

/// Test deleting photos with empty photo_ids (validation error)
#[tokio::test]
async fn test_delete_photos_empty_ids() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "pdele";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request("DELETE", "/photo", &user, json!({ "photoIds": [] }));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}
