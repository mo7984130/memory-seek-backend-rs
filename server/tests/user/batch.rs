use axum::http::StatusCode;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// 测试批量获取用户信息成功
#[tokio::test]
async fn test_get_user_info_batch_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let user1 = auth::register_and_login(&app, "gba1").await;
    guard.track_user(&user1.id);

    let user2 = auth::register_and_login(&app, "gba2").await;
    guard.track_user(&user2.id);

    let req = auth::auth_request(
        "POST",
        "/batch",
        &user1,
        json!({"userIds": [user1.id, user2.id]}),
    );
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let resp: Value = serde_json::from_slice(&body_bytes).unwrap();

    let data = resp["data"].as_array().expect("data 应为数组");
    assert_eq!(data.len(), 2, "应返回 2 条用户信息");

    // 验证每个用户信息包含必要字段
    for item in data {
        assert!(
            item["userId"].as_str().is_some(),
            "应包含 userId 字段"
        );
        assert!(
            item["nickname"].as_str().is_some(),
            "应包含 nickname 字段"
        );
    }

    guard.cleanup().await;
}

/// 测试批量获取用户信息 - 空列表
#[tokio::test]
async fn test_get_user_info_batch_empty() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let user = auth::register_and_login(&app, "gbe").await;
    guard.track_user(&user.id);

    let req = auth::auth_request("POST", "/batch", &user, json!({"userIds": []}));
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let resp: Value = serde_json::from_slice(&body_bytes).unwrap();

    let data = resp["data"].as_array().expect("data 应为数组");
    assert!(data.is_empty(), "空列表应返回空数组");

    guard.cleanup().await;
}
