//! 人脸接口的请求构造 / 响应解析测试。

mod common;

use futures::executor::block_on;
use memory_seek_api::{DateTime, FaceId, FaceIds, Method, PersonId, TimeIdCursor, VisualId};
use serde_json::json;

use common::{admin_job, assert_authorized, body_json, client, empty_page, ok};

#[test]
fn face_queries_build_expected_requests() {
    let (client, transport) = client(vec![
        ok(json!([])),
        ok(empty_page()),
        ok(json!(null)),
        ok(json!(null)),
        ok(json!(null)),
        ok(json!({ "deletedFaceCount": 2 })),
    ]);

    let faces = block_on(memory_seek_api::api::visual::face::get_faces(
        &client,
        VisualId(1),
    ))
    .unwrap();
    assert!(faces.is_empty());

    let cursor = TimeIdCursor {
        time_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        id: VisualId(9),
    };
    let page = block_on(
        memory_seek_api::api::visual::face::get_unassigned_face_visuals(
            &client,
            Some(cursor.clone()),
            Some(10),
        ),
    )
    .unwrap();
    assert!(page.records.is_empty());

    block_on(memory_seek_api::api::visual::face::change_face_belonging(
        &client,
        FaceId(2),
        Some(PersonId(3)),
    ))
    .unwrap();
    block_on(memory_seek_api::api::visual::face::change_face_belonging(
        &client,
        FaceId(2),
        None,
    ))
    .unwrap();
    block_on(memory_seek_api::api::visual::face::delete_face(
        &client,
        FaceId(4),
    ))
    .unwrap();

    let deleted = block_on(memory_seek_api::api::visual::face::delete_faces_batch(
        &client,
        FaceIds::new(vec![FaceId(5), FaceId(6)]).unwrap(),
    ))
    .unwrap();
    assert_eq!(deleted.deleted_face_count, 2);

    let requests = transport.requests();
    assert_eq!(requests.len(), 6);

    assert_eq!(requests[0].method, Method::GET);
    assert_eq!(requests[0].url, "/api/visual/face/visual/1");
    assert_authorized(&requests[0]);

    assert_eq!(requests[1].method, Method::GET);
    assert!(
        requests[1]
            .url
            .starts_with("/api/visual/face/unassigned-visuals?")
    );
    assert!(
        requests[1]
            .url
            .contains(&format!("cursor={}", cursor.encode()))
    );
    assert!(requests[1].url.contains("size=10"));

    assert_eq!(requests[2].method, Method::POST);
    assert_eq!(requests[2].url, "/api/visual/face/feature/2/belonging/3");
    assert_eq!(requests[2].body, None);

    assert_eq!(requests[3].method, Method::POST);
    assert_eq!(requests[3].url, "/api/visual/face/feature/2/belonging");

    assert_eq!(requests[4].method, Method::DELETE);
    assert_eq!(requests[4].url, "/api/visual/face/feature/4");

    assert_eq!(requests[5].method, Method::DELETE);
    assert_eq!(requests[5].url, "/api/visual/face/feature");
    assert_eq!(body_json(&requests[5]), json!({ "faceIds": ["5", "6"] }));
}

#[test]
fn face_admin_endpoints_build_expected_requests() {
    let (client, transport) = client(vec![admin_job("face_full"), admin_job("face_incremental")]);

    block_on(memory_seek_api::api::visual::face::full_compute(&client)).unwrap();
    block_on(memory_seek_api::api::visual::face::incremental_compute(
        &client,
    ))
    .unwrap();

    let requests = transport.requests();
    assert_eq!(requests[0].method, Method::POST);
    assert_eq!(requests[0].url, "/api/admin/jobs");
    assert_eq!(
        body_json(&requests[0]),
        json!({ "kind": "face_full", "threshold": null })
    );
    assert_eq!(requests[1].method, Method::POST);
    assert_eq!(requests[1].url, "/api/admin/jobs");
    assert_eq!(
        body_json(&requests[1]),
        json!({ "kind": "face_incremental", "threshold": null })
    );
}
