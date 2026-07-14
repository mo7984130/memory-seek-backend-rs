#![cfg(feature = "photo")]

mod helpers;

use helpers::{app::build_test_router, auth, db::CleanupGuard, photo, photo_like};
use serde_json::Value;

/// 点赞照片成功
#[tokio::test]
async fn test_like_photo_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "likes";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match photo::common::upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let res = photo_like::like_photo(&app, &user, &photo_id).await;
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body: Value = photo_like::parse_body(res).await;
    assert_eq!(body["code"], 200);

    guard.cleanup().await;
}

/// 取消点赞照片成功
#[tokio::test]
async fn test_unlike_photo_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "unlikes";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match photo::common::upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    // 先点赞
    photo_like::like_photo(&app, &user, &photo_id).await;

    // 取消点赞
    let res = photo_like::unlike_photo(&app, &user, &photo_id).await;
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body: Value = photo_like::parse_body(res).await;
    assert_eq!(body["code"], 200);

    guard.cleanup().await;
}

/// 重复点赞应返回错误
#[tokio::test]
async fn test_like_photo_already_liked() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "likedup";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match photo::common::upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    // 第一次点赞
    photo_like::like_photo(&app, &user, &photo_id).await;

    // 第二次点赞应该失败
    let res = photo_like::like_photo(&app, &user, &photo_id).await;
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body: Value = photo_like::parse_body(res).await;
    assert_eq!(body["code"], 400);
    assert!(
        body["msg"].as_str().unwrap().contains("已经点赞过"),
        "错误消息应包含'已经点赞过': {}",
        body["msg"]
    );

    guard.cleanup().await;
}

/// 取消未点赞的照片应返回错误
#[tokio::test]
async fn test_unlike_photo_not_liked() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "notliked";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match photo::common::upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    // 尝试取消点赞未点赞的照片
    let res = photo_like::unlike_photo(&app, &user, &photo_id).await;
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body: Value = photo_like::parse_body(res).await;
    assert_eq!(body["code"], 400);
    assert!(
        body["msg"].as_str().unwrap().contains("还未点赞"),
        "错误消息应包含'还未点赞': {}",
        body["msg"]
    );

    guard.cleanup().await;
}

/// 点赞不存在的照片应返回 404
#[tokio::test]
async fn test_like_nonexistent_photo() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "nonexphoto";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    // 尝试点赞不存在的照片
    let res = photo_like::like_photo(&app, &user, "999999").await;
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body: Value = photo_like::parse_body(res).await;
    assert_eq!(body["code"], 404);
    assert!(
        body["msg"].as_str().unwrap().contains("照片不存在"),
        "错误消息应包含'照片不存在': {}",
        body["msg"]
    );

    guard.cleanup().await;
}

/// 查询用户点赞的照片列表
#[tokio::test]
async fn test_get_user_liked_photos() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "likedlist";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    // 上传并点赞多张照片
    let photo_id1 = match photo::common::upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };
    photo_like::like_photo(&app, &user, &photo_id1).await;

    let photo_id2 = match photo::common::upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };
    photo_like::like_photo(&app, &user, &photo_id2).await;

    // 查询点赞列表
    let res = photo_like::get_liked_photos(&app, &user, None, 10).await;
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body: Value = photo_like::parse_body(res).await;
    assert_eq!(body["code"], 200);
    assert_eq!(
        body["data"]["records"].as_array().unwrap().len(),
        2,
        "应返回 2 条点赞记录"
    );

    guard.cleanup().await;
}
