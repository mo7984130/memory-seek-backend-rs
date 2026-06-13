use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use super::super::common::{publish_comment, upload_photo};
use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// Test getting comments when empty
#[tokio::test]
async fn test_get_comments_empty() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cgce";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let uri = format!("/photo/comment/{}?size=10", photo_id);
    let req = auth::auth_request("GET", &uri, &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    assert!(
        json["data"]["records"].as_array().unwrap().is_empty(),
        "无评论时应返回空列表"
    );

    guard.cleanup().await;
}

/// Test getting comments after publishing one
#[tokio::test]
async fn test_get_comments_after_publish() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cgca";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    publish_comment(&app, &user, &photo_id, "Great!").await;

    let uri = format!("/photo/comment/{}?size=10", photo_id);
    let req = auth::auth_request("GET", &uri, &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    let records = json["data"]["records"].as_array().unwrap();
    assert_eq!(records.len(), 1, "应有 1 条评论");
    assert_eq!(records[0]["content"], "Great!");

    guard.cleanup().await;
}

/// Test getting comments with pagination size
#[tokio::test]
async fn test_get_comments_pagination() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cgcp";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    // Publish 3 comments
    publish_comment(&app, &user, &photo_id, "Comment 1").await;
    publish_comment(&app, &user, &photo_id, "Comment 2").await;
    publish_comment(&app, &user, &photo_id, "Comment 3").await;

    // Get with size=2
    let uri = format!("/photo/comment/{}?size=2", photo_id);
    let req = auth::auth_request("GET", &uri, &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    let records = json["data"]["records"].as_array().unwrap();
    assert_eq!(records.len(), 2, "size=2 应返回 2 条评论");
    assert_eq!(json["data"]["hasMore"], true, "还有更多评论");

    guard.cleanup().await;
}
