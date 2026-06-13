use axum::http::StatusCode;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// 测试登出成功，登出后原 token 失效
#[tokio::test]
async fn test_logout_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "gulog";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    // 登出
    let req = auth::auth_request("POST", "/logout", &user, serde_json::json!(null));
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 用原 token 访问受保护路由，应返回 401
    let req = auth::auth_request("GET", "/me", &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "登出后原 token 应失效"
    );

    guard.cleanup().await;
}
