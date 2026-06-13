use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};
use super::super::common::create_collection;

/// Test updating collection name
#[tokio::test]
async fn test_update_collection_name() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cupdn";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let collection = create_collection(&app, &user, "Old Name", None).await;
    let collection_id = collection["id"].as_str().unwrap();

    let uri = format!("/photo/collections/{}", collection_id);
    let req = auth::auth_request(
        "PATCH",
        &uri,
        &user,
        serde_json::json!({ "name": "New Name" }),
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

/// Test updating collection description
#[tokio::test]
async fn test_update_collection_description() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cupdd";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let collection = create_collection(&app, &user, "Album", Some("Old desc")).await;
    let collection_id = collection["id"].as_str().unwrap();

    let uri = format!("/photo/collections/{}", collection_id);
    let req = auth::auth_request(
        "PATCH",
        &uri,
        &user,
        serde_json::json!({ "description": "New description" }),
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

/// Test updating collection with empty name (validation error)
#[tokio::test]
async fn test_update_collection_empty_name() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cupde";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let collection = create_collection(&app, &user, "Album", None).await;
    let collection_id = collection["id"].as_str().unwrap();

    let uri = format!("/photo/collections/{}", collection_id);
    let req = auth::auth_request(
        "PATCH",
        &uri,
        &user,
        serde_json::json!({ "name": "" }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}

/// Test updating collection name too long (validation error)
#[tokio::test]
async fn test_update_collection_name_too_long() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cupdl";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let collection = create_collection(&app, &user, "Album", None).await;
    let collection_id = collection["id"].as_str().unwrap();

    let long_name = "a".repeat(129);
    let uri = format!("/photo/collections/{}", collection_id);
    let req = auth::auth_request(
        "PATCH",
        &uri,
        &user,
        serde_json::json!({ "name": long_name }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}
