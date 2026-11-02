//! 管理端接口的请求构造 / 响应解析测试。

mod common;

use futures::executor::block_on;
use memory_seek_api::{Method, UserId};
use serde_json::json;

use common::{admin_job, body_json, client, empty_page, ok};

#[test]
fn admin_endpoints_build_expected_requests() {
    let (client, transport) = client(vec![
        ok(json!({
            "totalCount": 5,
            "totalBytes": 1234,
            "monthlyTimeline": [{ "dateStr": "2026-01", "count": 5 }]
        })),
        ok(empty_page()),
        ok(empty_page()),
        admin_job("face_incremental"),
        admin_job("face_incremental"),
        admin_job("face_incremental"),
    ]);

    let dashboard = block_on(memory_seek_api::api::admin::get_visual_dashboard(&client)).unwrap();
    assert_eq!(dashboard.total_count, 5);
    assert_eq!(dashboard.total_bytes, 1234);

    let users = block_on(memory_seek_api::api::admin::get_users(
        &client,
        Some(UserId(9)),
        Some(8),
    ))
    .unwrap();
    assert!(users.records.is_empty());

    let job_query = types_admin::job::AdminJobQuery {
        kind: Some(types_admin::job::AdminJobKind::FaceIncremental),
        status: Some(types_admin::job::AdminJobStatus::Running),
        cursor: None,
        size: 10,
    };
    let jobs = block_on(memory_seek_api::api::admin::get_jobs(&client, &job_query)).unwrap();
    assert!(jobs.records.is_empty());

    let started = block_on(memory_seek_api::api::admin::start_job(
        &client,
        &types_admin::job::StartAdminJobRequest {
            kind: types_admin::job::AdminJobKind::FaceIncremental,
            threshold: None,
        },
    ))
    .unwrap();
    assert_eq!(started.status, types_admin::job::AdminJobStatus::Queued);
    let _ = block_on(memory_seek_api::api::admin::get_job(&client, "job-1")).unwrap();
    let _ = block_on(memory_seek_api::api::admin::cancel_job(&client, "job-1")).unwrap();

    let requests = transport.requests();
    assert_eq!(requests[0].url, "/api/admin/dashboard/visuals");
    assert!(requests[1].url.starts_with("/api/admin/users?"));
    assert!(requests[1].url.contains("cursor=9"));
    assert!(requests[1].url.contains("size=8"));
    assert!(requests[2].url.starts_with("/api/admin/jobs?"));
    assert!(requests[2].url.contains("kind=face_incremental"));
    assert!(requests[2].url.contains("status=running"));
    assert_eq!(requests[3].method, Method::POST);
    assert_eq!(requests[3].url, "/api/admin/jobs");
    assert_eq!(
        body_json(&requests[3]),
        json!({ "kind": "face_incremental", "threshold": null })
    );
    assert_eq!(requests[4].url, "/api/admin/jobs/job-1");
    assert_eq!(requests[5].url, "/api/admin/jobs/job-1/cancel");
}
