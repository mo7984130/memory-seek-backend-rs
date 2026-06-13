use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use super::super::common::{publish_comment, upload_photo};
use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// Test unliking a comment successfully
#[tokio::test]
async fn test_unlike_comment_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cunlk";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let comment = publish_comment(&app, &user, &photo_id, "Like then unlike").await;
    let comment_id = comment["id"].as_str().unwrap();

    let uri = format!("/photo/comment/{}/{}/like", photo_id, comment_id);

    // Like first
    let req = auth::auth_request("POST", &uri, &user, serde_json::json!(null));
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Unlike
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

/// Test unliking a comment that was never liked
#[tokio::test]
async fn test_unlike_comment_not_liked() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cunlkn";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let comment = publish_comment(&app, &user, &photo_id, "Never liked").await;
    let comment_id = comment["id"].as_str().unwrap();

    let uri = format!("/photo/comment/{}/{}/like", photo_id, comment_id);
    let req = auth::auth_request("DELETE", &uri, &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "取消未点赞的评论应返回 400"
    );

    guard.cleanup().await;
}
