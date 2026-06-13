use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// 测试修改密码成功
#[tokio::test]
async fn test_change_password_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "gupw";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    // 修改密码
    let req = auth::auth_request(
        "PATCH",
        "/password",
        &user,
        json!({"oldPassword": user.password, "newPassword": "NewPass123"}),
    );
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 用新密码登录
    let login_body = json!({
        "account": user.username,
        "password": "NewPass123"
    });
    let login_res = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/login")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&login_body).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(login_res.status().is_success(), "用新密码登录应成功");

    guard.cleanup().await;
}
