use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use crate::helpers::{
    app::build_test_router, auth::register_and_login, db::CleanupGuard, test_config,
};

/// 测试正常注册
#[tokio::test]
async fn test_register_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new_with_cleanup(&["testuser_rok"]).await;

    let suffix = "rok";
    let user = register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    // 验证用户信息
    assert!(!user.id.is_empty(), "用户 ID 不应为空");
    assert_eq!(user.username, format!("testuser_{}", suffix));

    guard.cleanup().await;
}

/// 测试重复邮箱注册
#[tokio::test]
async fn test_register_duplicate_email() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new_with_cleanup(&["testuser_dem"]).await;

    let suffix = "dem";
    let user = register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    // 尝试用相同邮箱再次注册
    let body = json!({
        "username": format!("another_{}", suffix),
        "email": user.email,
        "password": "Test1234",
        "nickname": "Another",
        "inviterCode": "DriftC",
        "emailVerifyCode": "ABC123"
    });

    // 预设验证码
    setup_email_verify_code(&user.email, "ABC123").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}

/// 测试无效密码注册
#[tokio::test]
async fn test_register_invalid_password() {
    let app = build_test_router().await;

    let body = json!({
        "username": "testuser_invalid_pw",
        "email": "invalid_pw@example.com",
        "password": "123",  // 太短
        "nickname": "Test",
        "inviterCode": "DriftC",
        "emailVerifyCode": "ABC123"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// 测试无效邮箱注册
#[tokio::test]
async fn test_register_invalid_email() {
    let app = build_test_router().await;

    let body = json!({
        "username": "testuser_invalid_em",
        "email": "not-an-email",
        "password": "Test1234",
        "nickname": "Test",
        "inviterCode": "DriftC",
        "emailVerifyCode": "ABC123"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// 测试错误的邮箱验证码
#[tokio::test]
async fn test_register_wrong_email_code() {
    let app = build_test_router().await;

    let email = "wrong_code@example.com";
    setup_email_verify_code(email, "CORRECT").await;

    let body = json!({
        "username": "testuser_wrong_code",
        "email": email,
        "password": "Test1234",
        "nickname": "Test",
        "inviterCode": "DriftC",
        "emailVerifyCode": "WRONG1"  // 错误的验证码
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// 测试无效邀请码
#[tokio::test]
async fn test_register_invalid_inviter_code() {
    let app = build_test_router().await;

    let email = "invalid_inviter@example.com";
    setup_email_verify_code(email, "ABC123").await;

    let body = json!({
        "username": "testuser_inviter",
        "email": email,
        "password": "Test1234",
        "nickname": "Test",
        "inviterCode": "INVALID",  // 无效邀请码
        "emailVerifyCode": "ABC123"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

async fn setup_email_verify_code(email: &str, code: &str) {
    use deadpool_redis::redis::AsyncCommands;

    let cfg = test_config();
    let redis_cfg = deadpool_redis::Config::from_url(&cfg.redis.url);
    let pool = redis_cfg
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("创建 Redis 连接池失败");
    let mut conn = pool.get().await.expect("获取 Redis 连接失败");

    let key = format!("a:v:e:{}", email);
    let _: () = conn
        .set_ex(&key, code, 600)
        .await
        .expect("设置邮箱验证码失败");
}
