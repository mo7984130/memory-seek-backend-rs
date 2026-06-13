use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// 测试生成邀请码成功
#[tokio::test]
async fn test_generate_inviter_code_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "guci";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request("POST", "/inviter-code", &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    // 验证邀请码格式：6 个字符
    let code = json["data"]["inviterCode"]
        .as_str()
        .expect("应返回 inviterCode 字段");
    assert_eq!(code.len(), 6, "邀请码应为 6 个字符");

    // 验证过期时间存在
    assert!(
        json["data"]["expireAt"].as_str().is_some(),
        "应返回 expireAt 字段"
    );

    guard.cleanup().await;
}
