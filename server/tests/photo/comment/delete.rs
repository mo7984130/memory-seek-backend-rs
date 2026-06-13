use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};
use super::super::common::{upload_photo, publish_comment};

/// Test deleting own comment successfully
#[tokio::test]
async fn test_delete_comment_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cdel";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let comment = publish_comment(&app, &user, &photo_id, "To delete").await;
    let comment_id = comment["id"].as_str().unwrap();

    let uri = format!("/photo/comment/{}/{}", photo_id, comment_id);
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

/// Test deleting a non-existent comment
#[tokio::test]
async fn test_delete_comment_not_found() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cmdelnf";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let uri = format!("/photo/comment/{}/999999999", photo_id);
    let req = auth::auth_request("DELETE", &uri, &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    // Deleting non-existent comment should return 400
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}
