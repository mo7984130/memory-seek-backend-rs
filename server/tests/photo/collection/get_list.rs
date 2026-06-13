use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};
use super::super::common::create_collection;

/// Test getting collection list when empty
#[tokio::test]
async fn test_get_collection_list_empty() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cgle";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request("GET", "/photo/collections", &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    let list = json["data"].as_array().unwrap();
    // 服务层会自动为新用户创建"我喜欢"收藏夹，所以列表不为空
    assert_eq!(list.len(), 1, "新用户应自动创建 1 个我喜欢收藏夹");
    assert_eq!(list[0]["name"], "我喜欢");

    guard.cleanup().await;
}

/// Test getting collection list after creating one
#[tokio::test]
async fn test_get_collection_list_after_create() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cglc";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    create_collection(&app, &user, "Test Album", None).await;

    let req = auth::auth_request("GET", "/photo/collections", &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    let list = json["data"].as_array().unwrap();
    assert_eq!(list.len(), 1, "应有 1 个相册");
    assert_eq!(list[0]["name"], "Test Album");

    guard.cleanup().await;
}

/// Test getting collection list with multiple collections
#[tokio::test]
async fn test_get_collection_list_multiple() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cglm";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    create_collection(&app, &user, "Album A", None).await;
    create_collection(&app, &user, "Album B", Some("Description B")).await;

    let req = auth::auth_request("GET", "/photo/collections", &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    let list = json["data"].as_array().unwrap();
    assert_eq!(list.len(), 2, "应有 2 个相册");

    guard.cleanup().await;
}
