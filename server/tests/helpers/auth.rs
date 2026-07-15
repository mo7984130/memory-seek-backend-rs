use axum::body::Body;
use axum::http::{Request, header};
use serde_json::{Value, json};
use tower::ServiceExt;

use super::test_config;

/// 测试用户
#[allow(dead_code)]
pub struct TestUser {
    pub id: String,
    pub username: String,
    pub email: String,
    pub password: String,
    pub access_token: String,
    pub refresh_token: String,
}

/// 注册并登录用户，返回已认证的 TestUser
///
/// 流程：
/// 1. 在 Redis 中预设邮箱验证码和邀请码
/// 2. 调用 /register 注册
/// 3. 调用 /login 登录获取 token
#[allow(dead_code)]
pub async fn register_and_login(app: &axum::Router, suffix: &str) -> TestUser {
    let username = format!("testuser_{}", suffix);
    let email = format!("test_{}@example.com", suffix);
    let password = "Test1234".to_string();
    let nickname = format!("Test_{}", suffix);
    let inviter_code = "DriftC".to_string(); // 硬编码的邀请码
    let email_verify_code = "ABC123".to_string();

    // 预设邮箱验证码到 Redis（直接通过数据库连接设置）
    setup_email_verify_code(&email, &email_verify_code).await;

    // 注册
    let register_body = json!({
        "username": username,
        "email": email,
        "password": password,
        "nickname": nickname,
        "inviterCode": inviter_code,
        "emailVerifyCode": email_verify_code
    });

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&register_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let register_status = register_response.status();
    let register_body = axum::body::to_bytes(register_response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let register_json: Value = serde_json::from_slice(&register_body).unwrap();

    assert!(
        register_status.is_success(),
        "注册失败: status={}, body={}",
        register_status,
        register_json
    );

    let user_id = register_json["data"]["id"]
        .as_str()
        .expect("注册响应缺少 id")
        .to_string();

    // 登录
    let login_body = json!({
        "account": username,
        "password": password
    });

    let login_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&login_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let login_status = login_response.status();
    let login_body_bytes = axum::body::to_bytes(login_response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let login_json: Value = serde_json::from_slice(&login_body_bytes).unwrap();

    assert!(
        login_status.is_success(),
        "登录失败: status={}, body={}",
        login_status,
        login_json
    );

    let access_token = login_json["data"]["accessToken"]
        .as_str()
        .expect("登录响应缺少 accessToken")
        .to_string();
    let refresh_token = login_json["data"]["refreshToken"]
        .as_str()
        .expect("登录响应缺少 refreshToken")
        .to_string();

    TestUser {
        id: user_id,
        username,
        email,
        password,
        access_token,
        refresh_token,
    }
}

/// 构建带认证的请求
#[allow(dead_code)]
pub fn auth_request(method: &str, uri: &str, user: &TestUser, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {} {}", user.id, user.access_token),
        )
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}

/// 从 Redis 中直接设置邮箱验证码
#[allow(dead_code)]
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
