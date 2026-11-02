use std::time::Duration;

use common_core::{
    ContextualResult, error::contextual::ext::IntoContextualExt, time::now, types::CursorPage,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
    sea_query::{Expr, LockBehavior, LockType},
};
use types_admin::{
    job::{AdminJobKind, AdminJobQuery, AdminJobStatus},
    job_entity::{ActiveModel, Column, Entity, Model},
};
use types_core::cursor::TimeIdCursor;
use uuid::Uuid;

/// 心跳结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatOutcome {
    /// 心跳成功，且没有收到取消请求。
    Alive,
    /// 心跳成功，但数据库中已请求取消。
    CancelRequested,
    /// 任务已不再由当前执行者持有（被回收、被放弃或写入被拒绝）。
    Lost,
}

/// 写入一条排队任务。
///
/// 返回 `DbErr` 而不是上下文化错误，便于调用方识别唯一索引冲突并映射为 409。
pub async fn insert_queued(
    db: &impl ConnectionTrait,
    kind: AdminJobKind,
    requested_by: i64,
    payload: serde_json::Value,
) -> std::result::Result<Model, DbErr> {
    ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        kind: Set(kind),
        status: Set(AdminJobStatus::Queued),
        requested_by: Set(requested_by),
        payload: Set(payload),
        attempts: Set(0),
        cancel_requested: Set(false),
        created_at: Set(now()),
        ..Default::default()
    }
    .insert(db)
    .await
}

/// 判断错误是否为「同类型活动任务已存在」的唯一索引冲突。
pub fn is_active_conflict(error: &DbErr) -> bool {
    matches!(error, DbErr::Exec(_) | DbErr::Query(_))
        && error.to_string().contains("uniq_admin_job_active_kind")
}

/// 原子领取一个排队任务。
///
/// 使用 `FOR UPDATE SKIP LOCKED`，多个实例可同时安全调用而不会领取到同一条记录。
pub async fn claim_next(
    db: &DatabaseConnection,
    worker_id: &str,
) -> ContextualResult<Option<Model>> {
    let txn = db.begin().await.into_contextual()?;
    let candidate = claim_select().one(&txn).await.into_contextual()?;

    let Some(row) = candidate else {
        txn.commit().await.into_contextual()?;
        return Ok(None);
    };

    let attempts = row.attempts + 1;
    let mut active: ActiveModel = row.into();
    active.status = Set(AdminJobStatus::Running);
    active.worker_id = Set(Some(worker_id.to_owned()));
    active.run_id = Set(Some(Uuid::new_v4().to_string()));
    active.started_at = Set(Some(now()));
    active.finished_at = Set(None);
    active.attempts = Set(attempts);

    let row = active.update(&txn).await.into_contextual()?;
    // 心跳起始时间用数据库时钟，避免实例间时钟偏差影响失联判定。
    Entity::update_many()
        .col_expr(Column::HeartbeatAt, Expr::current_timestamp())
        .filter(Column::Id.eq(&row.id))
        .exec(&txn)
        .await
        .into_contextual()?;
    txn.commit().await.into_contextual()?;
    Ok(Some(row))
}

/// 领取候选查询；`SKIP LOCKED` 让多个实例可以在同一张表上并行领取。
fn claim_select() -> sea_orm::Select<Entity> {
    Entity::find()
        .filter(Column::Status.eq(AdminJobStatus::Queued))
        .filter(Column::CancelRequested.eq(false))
        .order_by_asc(Column::CreatedAt)
        .order_by_asc(Column::Id)
        .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
}

/// 写入心跳并检查取消请求。
pub async fn heartbeat(
    db: &DatabaseConnection,
    id: &str,
    run_id: &str,
) -> ContextualResult<HeartbeatOutcome> {
    let updated = Entity::update_many()
        .col_expr(Column::HeartbeatAt, Expr::current_timestamp())
        .filter(Column::Id.eq(id))
        .filter(Column::RunId.eq(run_id))
        .filter(Column::Status.is_in([AdminJobStatus::Running, AdminJobStatus::CancelRequested]))
        .exec(db)
        .await
        .into_contextual()?;

    if updated.rows_affected == 0 {
        return Ok(HeartbeatOutcome::Lost);
    }

    let cancel_requested = Entity::find_by_id(id)
        .select_only()
        .column(Column::CancelRequested)
        .into_tuple::<bool>()
        .one(db)
        .await
        .into_contextual()?
        .unwrap_or(false);

    Ok(if cancel_requested {
        HeartbeatOutcome::CancelRequested
    } else {
        HeartbeatOutcome::Alive
    })
}

/// 写入终态结果；仅当任务仍由当前 `run_id` 持有时生效。
pub async fn finish(
    db: &DatabaseConnection,
    id: &str,
    run_id: &str,
    status: AdminJobStatus,
    result: Option<serde_json::Value>,
    error: Option<String>,
) -> ContextualResult<bool> {
    let updated = Entity::update_many()
        .col_expr(Column::Status, Expr::value(status))
        .col_expr(Column::Result, Expr::value(result))
        .col_expr(Column::Error, Expr::value(truncate_error(error)))
        .col_expr(Column::FinishedAt, Expr::current_timestamp())
        .col_expr(Column::WorkerId, Expr::value(Option::<String>::None))
        .col_expr(Column::RunId, Expr::value(Option::<String>::None))
        .filter(Column::Id.eq(id))
        .filter(Column::RunId.eq(run_id))
        .filter(Column::Status.is_in([AdminJobStatus::Running, AdminJobStatus::CancelRequested]))
        .exec(db)
        .await
        .into_contextual()?;
    Ok(updated.rows_affected > 0)
}

/// `error` 列长度为 2048，写入前截断，避免超长错误导致终态写入失败。
fn truncate_error(error: Option<String>) -> Option<String> {
    const MAX_CHARS: usize = 2048;
    error.map(|error| {
        if error.chars().count() <= MAX_CHARS {
            error
        } else {
            error.chars().take(MAX_CHARS).collect()
        }
    })
}

/// 优雅关闭时释放任务，让其它实例立即接手。
pub async fn release(db: &DatabaseConnection, id: &str, run_id: &str) -> ContextualResult<bool> {
    let updated = Entity::update_many()
        .col_expr(Column::Status, Expr::value(AdminJobStatus::Queued))
        .col_expr(Column::WorkerId, Expr::value(Option::<String>::None))
        .col_expr(Column::RunId, Expr::value(Option::<String>::None))
        .filter(Column::Id.eq(id))
        .filter(Column::RunId.eq(run_id))
        .filter(Column::Status.is_in([AdminJobStatus::Running, AdminJobStatus::CancelRequested]))
        .exec(db)
        .await
        .into_contextual()?;
    Ok(updated.rows_affected > 0)
}

/// 管理员人工放弃一个运行中的任务（用于进程存活但任务卡死）。
pub async fn abandon(
    db: &impl ConnectionTrait,
    id: &str,
    reason: Option<String>,
) -> ContextualResult<bool> {
    let reason = truncate_error(reason).unwrap_or_else(|| "abandoned_by_admin".to_owned());
    let updated = Entity::update_many()
        .col_expr(Column::Status, Expr::value(AdminJobStatus::Failed))
        .col_expr(Column::Error, Expr::value(Some(reason)))
        .col_expr(Column::FinishedAt, Expr::current_timestamp())
        .col_expr(Column::WorkerId, Expr::value(Option::<String>::None))
        .col_expr(Column::RunId, Expr::value(Option::<String>::None))
        .filter(Column::Id.eq(id))
        .filter(Column::Status.is_in([AdminJobStatus::Running, AdminJobStatus::CancelRequested]))
        .exec(db)
        .await
        .into_contextual()?;
    Ok(updated.rows_affected > 0)
}

/// 把「已请求取消但仍处于 queued」的任务收敛为 cancelled。
pub async fn sweep_queued_cancellations(db: &DatabaseConnection) -> ContextualResult<u64> {
    let updated = Entity::update_many()
        .col_expr(Column::Status, Expr::value(AdminJobStatus::Cancelled))
        .col_expr(Column::FinishedAt, Expr::current_timestamp())
        .filter(Column::Status.eq(AdminJobStatus::Queued))
        .filter(Column::CancelRequested.eq(true))
        .exec(db)
        .await
        .into_contextual()?;
    Ok(updated.rows_affected)
}

/// 回收心跳中断的任务：
/// - `cancel_requested` 的执行者失联 → `cancelled`；
/// - `running` 的执行者失联 → `failed(instance_lost)`，等待人工重新触发。
pub async fn reap_stale(db: &DatabaseConnection, stale_after: Duration) -> ContextualResult<u64> {
    // `stale_after` 来自配置的 u64，直接格式化进 SQL，无注入风险。
    let cutoff = Expr::cust(format!(
        "heartbeat_at < current_timestamp - make_interval(secs => {})",
        stale_after.as_secs()
    ));

    let cancelled = Entity::update_many()
        .col_expr(Column::Status, Expr::value(AdminJobStatus::Cancelled))
        .col_expr(Column::FinishedAt, Expr::current_timestamp())
        .col_expr(Column::WorkerId, Expr::value(Option::<String>::None))
        .col_expr(Column::RunId, Expr::value(Option::<String>::None))
        .filter(Column::Status.eq(AdminJobStatus::CancelRequested))
        .filter(cutoff.clone())
        .exec(db)
        .await
        .into_contextual()?;

    let failed = Entity::update_many()
        .col_expr(Column::Status, Expr::value(AdminJobStatus::Failed))
        .col_expr(Column::Error, Expr::value(Some("instance_lost".to_owned())))
        .col_expr(Column::FinishedAt, Expr::current_timestamp())
        .col_expr(Column::WorkerId, Expr::value(Option::<String>::None))
        .col_expr(Column::RunId, Expr::value(Option::<String>::None))
        .filter(Column::Status.eq(AdminJobStatus::Running))
        .filter(cutoff)
        .exec(db)
        .await
        .into_contextual()?;

    Ok(cancelled.rows_affected + failed.rows_affected)
}

/// 按主键查询任务。
pub async fn find_by_id(db: &impl ConnectionTrait, id: &str) -> ContextualResult<Option<Model>> {
    Entity::find_by_id(id).one(db).await.into_contextual()
}

/// 直接取消一个排队任务；返回是否命中。
pub async fn cancel_queued(db: &impl ConnectionTrait, id: &str) -> ContextualResult<bool> {
    let updated = Entity::update_many()
        .col_expr(Column::Status, Expr::value(AdminJobStatus::Cancelled))
        .col_expr(Column::FinishedAt, Expr::current_timestamp())
        .filter(Column::Id.eq(id))
        .filter(Column::Status.eq(AdminJobStatus::Queued))
        .exec(db)
        .await
        .into_contextual()?;
    Ok(updated.rows_affected > 0)
}

/// 对运行中的任务发起取消；返回是否命中。
pub async fn request_cancel(db: &impl ConnectionTrait, id: &str) -> ContextualResult<bool> {
    let updated = Entity::update_many()
        .col_expr(Column::Status, Expr::value(AdminJobStatus::CancelRequested))
        .col_expr(Column::CancelRequested, Expr::value(true))
        .filter(Column::Id.eq(id))
        .filter(Column::Status.eq(AdminJobStatus::Running))
        .exec(db)
        .await
        .into_contextual()?;
    Ok(updated.rows_affected > 0)
}

/// 按查询条件返回一页任务（多取一条用于判断是否还有下一页）。
pub async fn list(
    db: &impl ConnectionTrait,
    query: &AdminJobQuery,
) -> ContextualResult<CursorPage<Model, TimeIdCursor<String>>> {
    let mut select = Entity::find();
    if let Some(kind) = query.kind {
        select = select.filter(Column::Kind.eq(kind));
    }
    if let Some(status) = query.status {
        select = select.filter(Column::Status.eq(status));
    }
    if let Some(cursor) = query.cursor.as_ref() {
        select = select.filter(
            Condition::any()
                .add(Column::CreatedAt.lt(cursor.time_at))
                .add(
                    Condition::all()
                        .add(Column::CreatedAt.eq(cursor.time_at))
                        .add(Column::Id.lt(cursor.id.clone())),
                ),
        );
    }
    let rows = select
        .order_by_desc(Column::CreatedAt)
        .order_by_desc(Column::Id)
        .limit(query.size + 1)
        .all(db)
        .await
        .into_contextual()?;

    Ok(
        CursorPage::from_oversize(rows, query.size).with_next_cursor(|row| TimeIdCursor {
            time_at: row.created_at,
            id: row.id.clone(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, QueryTrait};

    #[test]
    fn claim_query_uses_skip_locked() {
        let sql = claim_select().build(DbBackend::Postgres).to_string();
        assert!(
            sql.contains("FOR UPDATE SKIP LOCKED"),
            "领取查询必须使用 SKIP LOCKED: {sql}"
        );
        assert!(sql.contains("ORDER BY \"admin_job\".\"created_at\" ASC"));
    }

    #[test]
    fn unique_conflict_is_recognized() {
        let error = DbErr::Query(sea_orm::RuntimeErr::Internal(
            "duplicate key value violates unique constraint \"uniq_admin_job_active_kind\""
                .to_owned(),
        ));
        assert!(is_active_conflict(&error));

        let other = DbErr::Query(sea_orm::RuntimeErr::Internal("timeout".to_owned()));
        assert!(!is_active_conflict(&other));
    }
}

/// 需要真实 PostgreSQL 的多实例行为测试。
///
/// 运行方式：
/// ```text
/// ADMIN_TEST_DB='postgres://user:pass@127.0.0.1:5432/memory_seek_test' \
///   cargo test -p admin --features face -- --ignored
/// ```
///
/// 测试会 `DROP TABLE admin_job`，因此只允许连接名称包含 `test` 的数据库。
#[cfg(test)]
mod postgres_tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Schema};
    use types_admin::job::AdminJobKind;
    use types_admin::job_entity::Entity;

    #[tokio::test]
    #[ignore = "需要 ADMIN_TEST_DB 指向专用 PostgreSQL 测试库"]
    async fn multi_instance_job_lifecycle() {
        let url = std::env::var("ADMIN_TEST_DB").expect("需要设置 ADMIN_TEST_DB");
        assert!(
            url.contains("test"),
            "拒绝在非测试库上执行（ADMIN_TEST_DB 必须包含 test）"
        );
        let db = Database::connect(&url).await.unwrap();
        db.execute_unprepared("DROP TABLE IF EXISTS admin_job")
            .await
            .unwrap();
        let schema = Schema::new(DbBackend::Postgres);
        db.execute(&schema.create_table_from_entity(Entity))
            .await
            .unwrap();
        types_admin::job_entity::create_indexes(&db).await.unwrap();

        // 1. 唯一活动任务约束
        let first = insert_queued(&db, AdminJobKind::FaceFull, 1, serde_json::json!({}))
            .await
            .unwrap();
        let conflict = insert_queued(&db, AdminJobKind::FaceFull, 1, serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(is_active_conflict(&conflict), "{conflict:?}");

        // 2. 并发领取只成功一次
        let mut handles = Vec::new();
        for _ in 0..8 {
            let db = db.clone();
            handles.push(tokio::spawn(async move {
                claim_next(&db, "worker-test").await.unwrap()
            }));
        }
        let mut claimed = Vec::new();
        for handle in handles {
            if let Some(job) = handle.await.unwrap() {
                claimed.push(job);
            }
        }
        assert_eq!(claimed.len(), 1, "同一任务只能被领取一次");
        let job = claimed.pop().unwrap();
        assert_eq!(job.id, first.id);
        assert_eq!(job.attempts, 1);
        let run_id = job.run_id.clone().unwrap();

        // 3. 心跳与取消传导
        assert_eq!(
            heartbeat(&db, &job.id, &run_id).await.unwrap(),
            HeartbeatOutcome::Alive
        );
        db.execute_unprepared(&format!(
            "UPDATE admin_job SET cancel_requested = true WHERE id = '{}'",
            job.id
        ))
        .await
        .unwrap();
        assert_eq!(
            heartbeat(&db, &job.id, &run_id).await.unwrap(),
            HeartbeatOutcome::CancelRequested
        );

        // 4. fencing: 旧 run_id 写入被拒绝
        assert!(
            !finish(
                &db,
                &job.id,
                "wrong-run",
                AdminJobStatus::Succeeded,
                None,
                None
            )
            .await
            .unwrap()
        );
        assert!(
            finish(
                &db,
                &job.id,
                &run_id,
                AdminJobStatus::Cancelled,
                None,
                Some("cancelled".into())
            )
            .await
            .unwrap()
        );

        // 5. 优雅释放后重新领取，attempts 递增
        let second = insert_queued(&db, AdminJobKind::BackupManual, 1, serde_json::json!({}))
            .await
            .unwrap();
        let job = claim_next(&db, "worker-a").await.unwrap().unwrap();
        let run_id = job.run_id.clone().unwrap();
        assert!(release(&db, &second.id, &run_id).await.unwrap());
        let reclaimed = claim_next(&db, "worker-b").await.unwrap().unwrap();
        assert_eq!(reclaimed.id, second.id);
        assert_eq!(reclaimed.attempts, 2);

        // 6. 心跳中断后自动失败，需人工重试
        db.execute_unprepared(&format!(
            "UPDATE admin_job SET heartbeat_at = now() - interval '1 minute' WHERE id = '{}'",
            reclaimed.id
        ))
        .await
        .unwrap();
        assert!(reap_stale(&db, Duration::from_secs(30)).await.unwrap() >= 1);
        let row = find_by_id(&db, &reclaimed.id).await.unwrap().unwrap();
        assert_eq!(row.status, AdminJobStatus::Failed);
        assert_eq!(row.error.as_deref(), Some("instance_lost"));

        // 6b. 已请求取消且执行者失联 → cancelled
        let cancelling = insert_queued(
            &db,
            AdminJobKind::FaceSecondaryCluster,
            1,
            serde_json::json!({ "threshold": 0.7 }),
        )
        .await
        .unwrap();
        claim_next(&db, "worker-cancel").await.unwrap().unwrap();
        db.execute_unprepared(&format!(
            "UPDATE admin_job SET status = 'cancel_requested', cancel_requested = true, \
             heartbeat_at = now() - interval '1 minute' WHERE id = '{}'",
            cancelling.id
        ))
        .await
        .unwrap();
        assert!(reap_stale(&db, Duration::from_secs(30)).await.unwrap() >= 1);
        let row = find_by_id(&db, &cancelling.id).await.unwrap().unwrap();
        assert_eq!(row.status, AdminJobStatus::Cancelled);

        // 7. 失败后同类型可以重新创建
        let retried = insert_queued(&db, AdminJobKind::BackupManual, 1, serde_json::json!({}))
            .await
            .unwrap();

        // 8. 排队中被取消的任务由 supervisor 收敛为 cancelled
        db.execute_unprepared(&format!(
            "UPDATE admin_job SET cancel_requested = true WHERE id = '{}'",
            retried.id
        ))
        .await
        .unwrap();
        assert_eq!(sweep_queued_cancellations(&db).await.unwrap(), 1);
        let row = find_by_id(&db, &retried.id).await.unwrap().unwrap();
        assert_eq!(row.status, AdminJobStatus::Cancelled);

        // 9. 管理员人工放弃运行中的任务
        let abandoned = insert_queued(&db, AdminJobKind::FaceIncremental, 1, serde_json::json!({}))
            .await
            .unwrap();
        claim_next(&db, "worker-c").await.unwrap().unwrap();
        assert!(
            abandon(&db, &abandoned.id, Some("hung".into()))
                .await
                .unwrap()
        );
        let row = find_by_id(&db, &abandoned.id).await.unwrap().unwrap();
        assert_eq!(row.status, AdminJobStatus::Failed);
        assert_eq!(row.error.as_deref(), Some("hung"));
        assert!(row.run_id.is_none());

        db.execute_unprepared("DROP TABLE admin_job").await.unwrap();
    }
}
