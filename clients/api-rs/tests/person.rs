//! 人物接口的请求构造 / 响应解析测试。

mod common;

use futures::executor::block_on;
use memory_seek_api::{CountIdCursor, Method, PersonId, PersonName, SecondaryClusterParam};
use serde_json::json;

use common::{admin_job, body_json, client, empty_page_with_total, ok};

#[test]
fn person_endpoints_build_expected_requests() {
    let (client, transport) = client(vec![
        ok(empty_page_with_total()),
        ok(empty_page_with_total()),
        ok(empty_page_with_total()),
        ok(json!(null)),
        ok(json!(null)),
        ok(json!({
            "id": "10",
            "name": "Alice",
            "coverToken": "cover",
            "faceCount": 3
        })),
        admin_job("person_full_scan"),
        admin_job("face_secondary_cluster"),
    ]);

    let cursor = CountIdCursor {
        count: 5,
        id: PersonId(3),
    };
    let persons = block_on(memory_seek_api::api::visual::person::get_persons(
        &client,
        Some(cursor.clone()),
        Some(20),
    ))
    .unwrap();
    assert!(persons.page.records.is_empty());

    let page = block_on(memory_seek_api::api::visual::person::search_persons(
        &client,
        "ZS",
        Some(PersonId(9)),
        None,
    ))
    .unwrap();
    assert!(page.page.records.is_empty());

    let page = block_on(memory_seek_api::api::visual::person::get_person_visuals(
        &client,
        PersonId(4),
        None,
        Some(50),
    ))
    .unwrap();
    assert!(page.page.records.is_empty());

    block_on(memory_seek_api::api::visual::person::rename_person(
        &client,
        PersonId(4),
        PersonName::new("张三".to_owned()).unwrap(),
    ))
    .unwrap();

    block_on(memory_seek_api::api::visual::person::delete_person(
        &client,
        PersonId(4),
    ))
    .unwrap();

    let merged = block_on(memory_seek_api::api::visual::person::merge_person(
        &client,
        PersonId(1),
        PersonId(2),
    ))
    .unwrap();
    assert_eq!(merged.id, PersonId(10));
    assert_eq!(merged.name, "Alice");

    block_on(memory_seek_api::api::visual::person::full_scan(&client)).unwrap();
    block_on(memory_seek_api::api::visual::person::secondary_cluster(
        &client,
        &SecondaryClusterParam { threshold: 0.6 },
    ))
    .unwrap();

    let requests = transport.requests();
    assert_eq!(requests.len(), 8);

    assert_eq!(requests[0].method, Method::GET);
    assert!(requests[0].url.starts_with("/api/visual/person?"));
    assert!(
        requests[0]
            .url
            .contains(&format!("cursor={}", cursor.encode()))
    );
    assert!(requests[0].url.contains("size=20"));

    assert_eq!(requests[1].method, Method::GET);
    assert!(requests[1].url.starts_with("/api/visual/person/search?"));
    assert!(requests[1].url.contains("keyword=ZS"));
    assert!(requests[1].url.contains("cursor=9"));
    assert!(requests[1].url.contains("size=32"));

    assert_eq!(requests[2].method, Method::GET);
    assert_eq!(requests[2].url, "/api/visual/person/4/visuals?size=50");

    assert_eq!(requests[3].method, Method::POST);
    assert_eq!(requests[3].url, "/api/visual/person/4/name");
    assert_eq!(body_json(&requests[3]), json!({ "newName": "张三" }));

    assert_eq!(requests[4].method, Method::DELETE);
    assert_eq!(requests[4].url, "/api/admin/visual/person/4");

    assert_eq!(requests[5].method, Method::POST);
    assert_eq!(requests[5].url, "/api/admin/visual/person/merge");
    assert_eq!(
        body_json(&requests[5]),
        json!({ "sourcePersonId": "1", "targetPersonId": "2" })
    );

    assert_eq!(requests[6].method, Method::POST);
    assert_eq!(requests[6].url, "/api/admin/jobs");
    assert_eq!(
        body_json(&requests[6]),
        json!({ "kind": "person_full_scan" })
    );

    assert_eq!(requests[7].method, Method::POST);
    assert_eq!(requests[7].url, "/api/admin/jobs");
    assert_eq!(
        body_json(&requests[7]),
        json!({ "kind": "face_secondary_cluster", "threshold": 0.6 })
    );
}
