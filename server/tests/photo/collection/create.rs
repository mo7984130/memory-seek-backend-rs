use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// Test creating a collection successfully
#[tokio::test]
async fn test_create_collection_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "ccrt";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request(
        "POST",
        "/photo/collections",
        &user,
        serde_json::json!({ "name": "My Album" }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    assert!(json["data"]["id"].as_str().is_some(), "应返回相册 id");
    assert_eq!(json["data"]["name"], "My Album");
    assert_eq!(json["data"]["isFavorite"], false);

    guard.cleanup().await;
}

/// Test creating a collection with description
#[tokio::test]
async fn test_create_collection_with_description() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "ccrtd";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request(
        "POST",
        "/photo/collections",
        &user,
        serde_json::json!({ "name": "Vacation", "description": "Summer 2025" }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    assert_eq!(json["data"]["name"], "Vacation");
    assert_eq!(json["data"]["description"], "Summer 2025");

    guard.cleanup().await;
}

/// Test creating a collection with empty name (validation error)
#[tokio::test]
async fn test_create_collection_empty_name() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "ccrte";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request(
        "POST",
        "/photo/collections",
        &user,
        serde_json::json!({ "name": "" }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}

/// Test creating a collection with name too long (validation error)
#[tokio::test]
async fn test_create_collection_name_too_long() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "ccrtl";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let long_name = "a".repeat(129);
    let req = auth::auth_request(
        "POST",
        "/photo/collections",
        &user,
        serde_json::json!({ "name": long_name }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}

/// Test creating a collection with description too long (validation error)
#[tokio::test]
async fn test_create_collection_description_too_long() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "ccrdl";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let long_desc = "a".repeat(513);
    let req = auth::auth_request(
        "POST",
        "/photo/collections",
        &user,
        serde_json::json!({ "name": "Album", "description": long_desc }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}
