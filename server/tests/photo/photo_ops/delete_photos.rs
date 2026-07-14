use axum::http::StatusCode;
use serde_json::{json, Value};
use tower::ServiceExt;

use super::super::common::{create_collection, get_collections, upload_photo};
use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// Test deleting photos successfully
#[tokio::test]
async fn test_delete_photos_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "pdel";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    // Upload a photo first
    let photo_id = match upload_photo(&app, &user).await {
        Some(id) => id,
        None => {
            guard.cleanup().await;
            return;
        }
    };

    // Add photo to a collection so we can verify photoCount decrement on delete
    let collection = create_collection(&app, &user, "Album", None).await;
    let collection_id = collection["id"].as_str().unwrap();
    let add_uri = format!("/photo/collections/{}/photos", collection_id);
    let req = auth::auth_request("POST", &add_uri, &user, json!({ "photoIds": [photo_id] }));
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Verify photoCount = 1 after add
    let collections = get_collections(&app, &user).await;
    let c = collections
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == collection_id)
        .unwrap();
    assert_eq!(c["photoCount"], 1);

    // Delete the photo
    let req = auth::auth_request("DELETE", "/photo", &user, json!({ "photoIds": [photo_id] }));
    let res = app.clone().oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["code"], 200);

    // Verify photoCount was decremented in the collection
    let collections = get_collections(&app, &user).await;
    let c = collections
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == collection_id)
        .unwrap();
    assert_eq!(c["photoCount"], 0);

    guard.cleanup().await;
}

/// Test deleting photos with empty photo_ids (validation error)
#[tokio::test]
async fn test_delete_photos_empty_ids() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "pdele";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let req = auth::auth_request("DELETE", "/photo", &user, json!({ "photoIds": [] }));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}
