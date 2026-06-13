use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};
use super::super::common::{upload_photo, publish_comment};

/// Test liking a comment successfully
#[tokio::test]
async fn test_like_comment_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "clike";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let comment = publish_comment(&app, &user, &photo_id, "Like me!").await;
    let comment_id = comment["id"].as_str().unwrap();

    let uri = format!("/photo/comment/{}/{}/like", photo_id, comment_id);
    let req = auth::auth_request("POST", &uri, &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["code"], 200);

    guard.cleanup().await;
}

/// Test liking a comment twice (should return error)
#[tokio::test]
async fn test_like_comment_duplicate() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cliked";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let comment = publish_comment(&app, &user, &photo_id, "Like me!").await;
    let comment_id = comment["id"].as_str().unwrap();

    let uri = format!("/photo/comment/{}/{}/like", photo_id, comment_id);

    // First like
    let req = auth::auth_request("POST", &uri, &user, serde_json::json!(null));
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Second like should fail
    let req = auth::auth_request("POST", &uri, &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST, "重复点赞应返回 400");

    guard.cleanup().await;
}

/// Test liking a non-existent comment
#[tokio::test]
async fn test_like_comment_not_found() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "clikenf";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let uri = format!("/photo/comment/{}/999999999/like", photo_id);
    let req = auth::auth_request("POST", &uri, &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    guard.cleanup().await;
}
