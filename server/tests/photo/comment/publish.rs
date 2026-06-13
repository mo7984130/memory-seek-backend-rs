use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};
use super::super::common::upload_photo;

/// Test publishing a comment successfully
#[tokio::test]
async fn test_publish_comment_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cpub";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let uri = format!("/photo/comment/{}", photo_id);
    let req = auth::auth_request(
        "POST",
        &uri,
        &user,
        serde_json::json!({ "content": "Nice photo!" }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    assert!(json["data"]["id"].as_str().is_some(), "应返回评论 id");
    assert_eq!(json["data"]["content"], "Nice photo!");
    assert_eq!(json["data"]["likeCount"], 0);
    assert_eq!(json["data"]["isLiked"], false);

    guard.cleanup().await;
}

/// Test publishing a comment with empty content (validation error)
#[tokio::test]
async fn test_publish_comment_empty_content() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cpube";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let uri = format!("/photo/comment/{}", photo_id);
    let req = auth::auth_request(
        "POST",
        &uri,
        &user,
        serde_json::json!({ "content": "" }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}

/// Test publishing a comment with content too long (validation error)
#[tokio::test]
async fn test_publish_comment_content_too_long() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cpubl";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let long_content = "a".repeat(1025);
    let uri = format!("/photo/comment/{}", photo_id);
    let req = auth::auth_request(
        "POST",
        &uri,
        &user,
        serde_json::json!({ "content": long_content }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}

/// Test publishing a comment on a non-existent photo
#[tokio::test]
async fn test_publish_comment_nonexistent_photo() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cpubnp";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request(
        "POST",
        "/photo/comment/999999999",
        &user,
        serde_json::json!({ "content": "Hello" }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}
