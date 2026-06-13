use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// 测试获取当前用户信息成功
#[tokio::test]
async fn test_get_user_info_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "guok";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request("GET", "/me", &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body_bytes);
    assert_eq!(status, StatusCode::OK, "GET /me failed: {}", body_str);
    let json: Value = serde_json::from_str(&body_str)
        .unwrap_or_else(|e| panic!("Failed to parse JSON: {}, body: {}", e, body_str));

    // 验证返回的用户数据
    let data = &json["data"];
    assert_eq!(data["id"].as_str().unwrap(), user.id);
    assert_eq!(data["username"].as_str().unwrap(), user.username);
    assert_eq!(data["email"].as_str().unwrap(), user.email);
    assert!(data["nickname"].as_str().is_some(), "应返回 nickname 字段");
    assert!(
        data["createdAt"].as_str().is_some(),
        "应返回 createdAt 字段"
    );

    guard.cleanup().await;
}
