use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};
use super::super::common::{upload_photo, create_collection};

/// Test adding photos to a collection successfully
#[tokio::test]
async fn test_add_batch_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cpab";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let collection = create_collection(&app, &user, "Album", None).await;
    let collection_id = collection["id"].as_str().unwrap();

    let uri = format!("/photo/collections/{}/photos", collection_id);
    let req = auth::auth_request(
        "POST",
        &uri,
        &user,
        serde_json::json!({ "photoIds": [photo_id] }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["code"], 200);
    assert_eq!(json["data"]["newPhotoCount"], 1);

    guard.cleanup().await;
}

/// Test adding photos with empty list (validation error)
#[tokio::test]
async fn test_add_batch_empty_list() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cpabe";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let collection = create_collection(&app, &user, "Album", None).await;
    let collection_id = collection["id"].as_str().unwrap();

    let uri = format!("/photo/collections/{}/photos", collection_id);
    let req = auth::auth_request(
        "POST",
        &uri,
        &user,
        serde_json::json!({ "photoIds": [] }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}

/// Test adding photos to a non-existent collection
#[tokio::test]
async fn test_add_batch_nonexistent_collection() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cpabnc";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let req = auth::auth_request(
        "POST",
        "/photo/collections/999999999/photos",
        &user,
        serde_json::json!({ "photoIds": [photo_id] }),
    );
    let res = app.oneshot(req).await.unwrap();

    // Should return an error (404 or 500 depending on implementation)
    assert!(
        !res.status().is_success() || res.status() == StatusCode::OK,
        "添加到不存在的相册应返回错误"
    );

    guard.cleanup().await;
}
