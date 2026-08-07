use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use sea_orm::ConnectionTrait;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::common::{MINIMAL_JPEG, multipart_upload_request};

/// 上传照片并返回完整响应 JSON（用于获取 preview_token 等）
async fn upload_photo_full(app: &axum::Router, user: &auth::TestUser) -> Option<Value> {
    let req = multipart_upload_request("/photo", user, MINIMAL_JPEG, "test.png");
    let res = app.clone().oneshot(req).await.unwrap();

    if res.status() == StatusCode::INTERNAL_SERVER_ERROR {
        return None;
    }
    assert_eq!(res.status(), StatusCode::OK, "上传照片失败");

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["code"], 200);
    Some(json["data"].clone())
}

/// 直连测试库查询行为记录数量
async fn behavior_count(user_id: &str, action: &str) -> i64 {
    let cfg = crate::helpers::test_config();
    let db = sea_orm::Database::connect(&cfg.database.url)
        .await
        .expect("连接测试数据库失败");

    let row = db
        .query_one(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT COUNT(*) AS cnt FROM photo_user_behavior \
             WHERE user_id = $1 AND action = $2",
            [user_id.parse::<i64>().unwrap().into(), action.into()],
        ))
        .await
        .expect("查询行为记录失败")
        .expect("查询无结果");

    let cnt: i64 = row.try_get("", "cnt").expect("解析行为记录数量失败");
    let _ = db.close().await;
    cnt
}

/// 上传照片应产生 upload 行为记录
#[tokio::test]
async fn test_upload_records_behavior() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let user = auth::register_and_login(&app, "beh_up").await;
    guard.track_user(&user.id);

    match upload_photo_full(&app, &user).await {
        Some(_) => {}
        None => {
            guard.cleanup().await;
            return;
        }
    }

    assert_eq!(
        behavior_count(&user.id, "upload").await,
        1,
        "上传后应产生 1 条 upload 行为记录"
    );

    guard.cleanup().await;
}

/// 点赞照片应产生 like 行为记录
#[tokio::test]
async fn test_like_records_behavior() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let user = auth::register_and_login(&app, "beh_like").await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo_full(&app, &user).await {
        Some(data) => data["id"].as_str().unwrap().to_string(),
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let req = auth::auth_request(
        "POST",
        &format!("/photo/photos/{photo_id}/like"),
        &user,
        serde_json::json!(null),
    );
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    assert_eq!(
        behavior_count(&user.id, "like").await,
        1,
        "点赞后应产生 1 条 like 行为记录"
    );

    guard.cleanup().await;
}

/// 预览图访问应产生 view 行为记录（缩略图不计入）
#[tokio::test]
async fn test_view_records_behavior() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let user = auth::register_and_login(&app, "beh_view").await;
    guard.track_user(&user.id);

    let data = match upload_photo_full(&app, &user).await {
        Some(data) => data,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let preview_token = match data["previewToken"].as_str() {
        Some(t) => t.to_string(),
        None => {
            guard.cleanup().await;
            return;
        }
    };

    // 访问预览图（公开路由）
    let req = Request::builder()
        .method("GET")
        .uri(format!("/photo/{preview_token}"))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK, "预览图访问失败");

    // view 为异步写入，稍等落库
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    assert_eq!(
        behavior_count(&user.id, "view").await,
        1,
        "预览图访问应产生 1 条 view 行为记录"
    );

    guard.cleanup().await;
}

/// 删除照片后，审计行为记录仍然保留
#[tokio::test]
async fn test_delete_photos_keeps_behavior() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let user = auth::register_and_login(&app, "beh_del").await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo_full(&app, &user).await {
        Some(data) => data["id"].as_str().unwrap().to_string(),
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let req = auth::auth_request("DELETE", "/photo", &user, json!({ "photoIds": [photo_id] }));
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 删除后 upload 与 delete_photos 记录均应保留
    assert_eq!(
        behavior_count(&user.id, "upload").await,
        1,
        "删除照片后 upload 行为记录应保留"
    );
    assert_eq!(
        behavior_count(&user.id, "delete_photos").await,
        1,
        "删除照片应产生 delete_photos 行为记录"
    );

    guard.cleanup().await;
}

/// 非管理员访问行为统计接口应返回 403
#[tokio::test]
async fn test_behavior_admin_forbidden_for_non_admin() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let user = auth::register_and_login(&app, "beh_nonadmin").await;
    guard.track_user(&user.id);

    let req = auth::auth_request(
        "GET",
        "/photo/admin/behaviors/stats",
        &user,
        serde_json::json!(null),
    );
    let res = app.clone().oneshot(req).await.unwrap();

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["code"], 403, "非管理员应返回 403: {}", json);

    guard.cleanup().await;
}

/// 管理员可查询行为量统计
#[tokio::test]
async fn test_behavior_admin_stats_success() {
    use constants::PasswordHasher;

    let app = build_test_router().await;
    let guard = CleanupGuard::new().await;

    let cfg = crate::helpers::test_config();
    let db = sea_orm::Database::connect(&cfg.database.url)
        .await
        .expect("连接测试数据库失败");

    // 备份并临时重置管理员(用户ID=1)密码
    let original_hash: Option<String> = db
        .query_one(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT password FROM auth_user WHERE id = 1",
            [],
        ))
        .await
        .expect("查询管理员失败")
        .and_then(|row| row.try_get("", "password").ok());

    let admin_password = "AdminPass123";
    let new_hash = PasswordHasher.hash(admin_password).expect("哈希密码失败");
    let _ = db
        .execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            "UPDATE auth_user SET password = $1 WHERE id = 1",
            [new_hash.into()],
        ))
        .await;

    // 管理员登录
    let login_body = json!({
        "account": "DriftCloud",
        "password": admin_password
    });
    let login_res = app
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
    let login_bytes = axum::body::to_bytes(login_res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let login_json: Value = serde_json::from_slice(&login_bytes).unwrap();
    assert_eq!(login_json["code"], 200, "管理员登录失败: {}", login_json);

    let admin = auth::TestUser {
        id: "1".to_string(),
        username: "DriftCloud".to_string(),
        email: "mo.drift.cloud@gmail.com".to_string(),
        password: admin_password.to_string(),
        access_token: login_json["data"]["accessToken"]
            .as_str()
            .unwrap()
            .to_string(),
        refresh_token: login_json["data"]["refreshToken"]
            .as_str()
            .unwrap()
            .to_string(),
    };

    // 查询行为量统计
    let req = auth::auth_request(
        "GET",
        "/photo/admin/behaviors/stats?granularity=day",
        &admin,
        serde_json::json!(null),
    );
    let res = app.clone().oneshot(req).await.unwrap();
    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["code"], 200, "管理员统计查询失败: {}", json);

    // 恢复管理员原密码
    let restore_sql = match &original_hash {
        Some(_hash) => "UPDATE auth_user SET password = $1 WHERE id = 1",
        None => "DELETE FROM auth_user WHERE id = 1",
    };
    let values: Vec<sea_orm::Value> = match &original_hash {
        Some(hash) => vec![hash.clone().into()],
        None => vec![],
    };
    let _ = db
        .execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            restore_sql,
            values,
        ))
        .await;
    let _ = db.close().await;

    guard.cleanup().await;
}
