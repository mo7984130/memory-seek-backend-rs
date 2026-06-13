use axum::http::StatusCode;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// Test checking MD5 existence with empty list (validation rejects min=1)
#[tokio::test]
async fn test_md5s_exist_empty() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "pmd5e";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request(
        "POST",
        "/photo/check-existence",
        &user,
        json!({ "md5s": [] }),
    );
    let res = app.oneshot(req).await.unwrap();

    // Md5sExistParam validates min=1, so empty vec returns 400
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}

/// Test checking MD5 existence with a non-existent hash
#[tokio::test]
async fn test_md5s_exist_not_found() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "pmd5n";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request(
        "POST",
        "/photo/check-existence",
        &user,
        json!({ "md5s": ["abc123"] }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    let results = json["data"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], false, "不存在的 md5 应返回 false");

    guard.cleanup().await;
}

/// Test checking MD5 existence with multiple hashes
#[tokio::test]
async fn test_md5s_exist_multiple() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "pmd5m";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request(
        "POST",
        "/photo/check-existence",
        &user,
        json!({ "md5s": ["hash_aaa", "hash_bbb", "hash_ccc"] }),
    );
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    let results = json["data"].as_array().unwrap();
    assert_eq!(results.len(), 3, "应返回 3 个结果");

    guard.cleanup().await;
}
