use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};
use super::super::common::{upload_photo, create_collection};

/// Test getting photos from an empty collection
#[tokio::test]
async fn test_get_photos_empty() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cpgpe";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let collection = create_collection(&app, &user, "Album", None).await;
    let collection_id = collection["id"].as_str().unwrap();

    let uri = format!("/photo/collections/{}/photos?size=10", collection_id);
    let req = auth::auth_request("GET", &uri, &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    assert!(
        json["data"]["records"].as_array().unwrap().is_empty(),
        "空相册应返回空记录"
    );

    guard.cleanup().await;
}

/// Test getting photos after adding to collection
#[tokio::test]
async fn test_get_photos_after_add() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cpgpa";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    let collection = create_collection(&app, &user, "Album", None).await;
    let collection_id = collection["id"].as_str().unwrap();

    // Add photo to collection
    let add_uri = format!("/photo/collections/{}/photos", collection_id);
    let req = auth::auth_request(
        "POST",
        &add_uri,
        &user,
        serde_json::json!({ "photoIds": [photo_id] }),
    );
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Get photos from collection
    let get_uri = format!("/photo/collections/{}/photos?size=10", collection_id);
    let req = auth::auth_request("GET", &get_uri, &user, serde_json::json!(null));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["code"], 200);
    let records = json["data"]["records"].as_array().unwrap();
    assert_eq!(records.len(), 1, "应有 1 张照片");
    assert_eq!(records[0]["id"].as_str().unwrap(), photo_id);

    guard.cleanup().await;
}
