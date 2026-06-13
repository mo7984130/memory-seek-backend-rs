use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// 测试修改昵称成功
#[tokio::test]
async fn test_change_nickname_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "gunc";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request(
        "PATCH",
        "/nickname",
        &user,
        json!({"newNickname": "NewName123"}),
    );
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    guard.cleanup().await;
}

/// 测试修改昵称包含非法字符
#[tokio::test]
async fn test_change_nickname_invalid_chars() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "gnci";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request(
        "PATCH",
        "/nickname",
        &user,
        json!({"newNickname": "bad<name>"}),
    );
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}
