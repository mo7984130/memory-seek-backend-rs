use axum::Router;
use serde_json::Value;
use tower::ServiceExt;

use super::auth;

/// 点赞照片
pub async fn like_photo(
    app: &Router,
    user: &auth::TestUser,
    photo_id: &str,
) -> axum::http::Response<axum::body::Body> {
    let uri = format!("/photo/likes/photos/{}/like", photo_id);
    let req = auth::auth_request("POST", &uri, user, serde_json::json!(null));
    app.clone().oneshot(req).await.unwrap()
}

/// 取消点赞照片
pub async fn unlike_photo(
    app: &Router,
    user: &auth::TestUser,
    photo_id: &str,
) -> axum::http::Response<axum::body::Body> {
    let uri = format!("/photo/likes/photos/{}/like", photo_id);
    let req = auth::auth_request("DELETE", &uri, user, serde_json::json!(null));
    app.clone().oneshot(req).await.unwrap()
}

/// 查询用户点赞的照片列表
pub async fn get_liked_photos(
    app: &Router,
    user: &auth::TestUser,
    cursor: Option<&str>,
    size: u64,
) -> axum::http::Response<axum::body::Body> {
    let mut uri = format!("/photo/likes/photos/liked?size={}", size);
    if let Some(c) = cursor {
        uri = format!("{}&cursor={}", uri, c);
    }

    let req = auth::auth_request("GET", &uri, user, serde_json::json!(null));
    app.clone().oneshot(req).await.unwrap()
}

/// 从响应中解析 JSON body
pub async fn parse_body(res: axum::http::Response<axum::body::Body>) -> Value {
    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    serde_json::from_slice(&body_bytes).unwrap()
}
