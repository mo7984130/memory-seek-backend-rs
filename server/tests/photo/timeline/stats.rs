use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use super::super::common::upload_photo;
use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// Test getting timeline stats when no photos exist
#[tokio::test]
async fn test_get_stats_empty() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "tse";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request(
        "GET",
        "/photo/timeline/stats",
        &user,
        serde_json::json!(null),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    // No photos → minTime and maxTime should be null
    assert!(
        json["data"]["minTime"].is_null(),
        "无照片时 minTime 应为 null"
    );
    assert!(
        json["data"]["maxTime"].is_null(),
        "无照片时 maxTime 应为 null"
    );

    guard.cleanup().await;
}

/// Test getting timeline stats after uploading a photo
#[tokio::test]
async fn test_get_stats_after_upload() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "tsa";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let _photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let req = auth::auth_request(
        "GET",
        "/photo/timeline/stats",
        &user,
        serde_json::json!(null),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    // After upload, minTime and maxTime should be non-null
    assert!(
        !json["data"]["minTime"].is_null(),
        "上传照片后 minTime 不应为 null"
    );
    assert!(
        !json["data"]["maxTime"].is_null(),
        "上传照片后 maxTime 不应为 null"
    );

    guard.cleanup().await;
}
