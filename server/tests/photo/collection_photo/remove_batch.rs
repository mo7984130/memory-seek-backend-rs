use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use super::super::common::{create_collection, get_collections, upload_photo};
use crate::helpers::{app::build_test_router, auth, db::CleanupGuard};

/// Test batch removing photos from collection
#[tokio::test]
async fn test_remove_batch_success() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cprb";
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

    // Batch remove photos
    let remove_uri = format!("/photo/collections/{}/photos", collection_id);
    let req = auth::auth_request(
        "DELETE",
        &remove_uri,
        &user,
        serde_json::json!({ "photoIds": [photo_id] }),
    );
    let res = app.clone().oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["code"], 200);
    assert_eq!(json["data"]["removedPhotoCount"], 1);

    // Verify photoCount was updated
    let collections = get_collections(&app, &user).await;
    let updated = collections
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == collection_id)
        .unwrap();
    assert_eq!(updated["photoCount"], 0);

    guard.cleanup().await;
}

/// Test batch removing with empty list (validation error)
#[tokio::test]
async fn test_remove_batch_empty_list() {
    let app = build_test_router().await;
    let mut guard = CleanupGuard::new().await;

    let suffix = "cprbe";
    let user = auth::register_and_login(&app, suffix).await;
    guard.track_user(&user.id);

    let collection = create_collection(&app, &user, "Album", None).await;
    let collection_id = collection["id"].as_str().unwrap();

    let uri = format!("/photo/collections/{}/photos", collection_id);
    let req = auth::auth_request("DELETE", &uri, &user, serde_json::json!({ "photoIds": [] }));
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    guard.cleanup().await;
}
