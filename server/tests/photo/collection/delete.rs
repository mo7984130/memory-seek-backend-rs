use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use super::super::common::create_collection;
use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// Test deleting a collection successfully
#[tokio::test]
async fn test_delete_collection_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cdel";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let collection = create_collection(&app, &user, "To Delete", None).await;
    let collection_id = collection["id"].as_str().unwrap();

    let uri = format!("/photo/collections/{}", collection_id);
    let req = auth::auth_request("DELETE", &uri, &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["code"], 200);

    guard.cleanup().await;
}

/// Test deleting a non-existent collection
#[tokio::test]
async fn test_delete_collection_not_found() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cdelnf";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    // Use a very large ID that shouldn't exist
    let req = auth::auth_request(
        "DELETE",
        "/photo/collections/999999999",
        &user,
        serde_json::json!(null),
    );
    let res = app.oneshot(req).await.unwrap();

    // Should return 404 or 500 depending on service implementation
    assert!(
        res.status() == StatusCode::NOT_FOUND || res.status() == StatusCode::INTERNAL_SERVER_ERROR,
        "删除不存在的相册应返回错误, got: {}",
        res.status()
    );

    guard.cleanup().await;
}
