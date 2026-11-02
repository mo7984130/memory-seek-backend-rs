//! 备份接口的请求构造 / 响应解析测试。

mod common;

use futures::executor::block_on;
use memory_seek_api::{BackupSource, BackupTier, Method, RestoreRequest};
use serde_json::json;

use common::{admin_job, body_json, client};

#[test]
fn backup_endpoints_build_expected_requests() {
    let (client, transport) = client(vec![
        admin_job("backup_scheduled"),
        admin_job("backup_manual"),
        admin_job("backup_restore"),
    ]);

    let scheduled = block_on(memory_seek_api::api::backup::trigger_backup(&client)).unwrap();
    assert_eq!(
        scheduled.kind,
        types_admin::job::AdminJobKind::BackupScheduled
    );

    let manual = block_on(memory_seek_api::api::backup::manual_backup(&client)).unwrap();
    assert_eq!(manual.kind, types_admin::job::AdminJobKind::BackupManual);

    let request = RestoreRequest {
        tier: BackupTier::Daily,
        source: BackupSource::Local,
        run_id: "run-1".to_owned(),
        confirm_run_id: "run-1".to_owned(),
    };
    let restored = block_on(memory_seek_api::api::backup::restore_backup(
        &client, &request,
    ))
    .unwrap();
    assert_eq!(restored.kind, types_admin::job::AdminJobKind::BackupRestore);

    let requests = transport.requests();
    assert_eq!(requests[0].method, Method::POST);
    assert_eq!(requests[0].url, "/api/admin/backup/trigger");
    assert_eq!(requests[1].url, "/api/admin/backup/manual");
    assert_eq!(requests[2].url, "/api/admin/backup/restore");
    assert_eq!(
        body_json(&requests[2]),
        json!({
            "tier": "daily",
            "source": "local",
            "runId": "run-1",
            "confirmRunId": "run-1"
        })
    );
}
