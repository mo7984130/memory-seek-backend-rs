use axum::Router;
use hyper::{Body, Request};
use tower::ServiceExt;

pub async fn like_photo(app: &Router, user_id: i64, photo_id: &str) -> hyper::Response<Body> {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/photo/likes/photos/{}/like", photo_id))
                .header("x-test-user-id", user_id.to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

pub async fn unlike_photo(app: &Router, user_id: i64, photo_id: &str) -> hyper::Response<Body> {
    app.clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/photo/likes/photos/{}/like", photo_id))
                .header("x-test-user-id", user_id.to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

pub async fn get_liked_photos(
    app: &Router,
    user_id: i64,
    cursor: Option<&str>,
    size: u64,
) -> hyper::Response<Body> {
    let mut uri = format!("/photo/likes/photos/liked?size={}", size);
    if let Some(c) = cursor {
        uri = format!("{}&cursor={}", uri, c);
    }

    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("x-test-user-id", user_id.to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}
