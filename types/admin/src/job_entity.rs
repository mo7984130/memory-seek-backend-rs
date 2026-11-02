use common_core::time::DateTime;
use sea_orm::entity::prelude::*;

use crate::job::{AdminJobKind, AdminJobStatus};

/// 后台任务表。
///
/// 多实例下由数据库承担全部协调职责：
/// - `uniq_admin_job_active_kind` 保证同类型最多一个活动任务；
/// - `run_id` 作为 fencing token，避免旧执行者覆盖新结果；
/// - `heartbeat_at` 用于识别失联实例，超时后自动标记失败；
/// - 执行参数保存在 `payload`，进程重启后可完整恢复。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "admin_job")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "String(StringLen::N(36))")]
    pub id: String,
    /// 标记 `indexed` 是为了让 SeaORM 的 schema sync 识别出 `kind` 上已存在索引：
    /// 真正的唯一约束是下方 `create_indexes` 用原生 SQL 建的部分唯一索引，
    /// 若不声明，sync 会把它当成多余的唯一索引并尝试 `DROP CONSTRAINT`。
    #[sea_orm(indexed)]
    pub kind: AdminJobKind,
    pub status: AdminJobStatus,
    pub requested_by: i64,
    /// 任务执行参数（如二次聚类阈值）。
    #[sea_orm(column_type = "JsonBinary")]
    pub payload: Json,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub result: Option<Json>,
    #[sea_orm(column_type = "String(StringLen::N(2048))", nullable)]
    pub error: Option<String>,
    /// 当前持有任务的实例标识，仅用于诊断。
    #[sea_orm(column_type = "String(StringLen::N(128))", nullable)]
    pub worker_id: Option<String>,
    /// 每次领取生成的 fencing token。
    #[sea_orm(column_type = "String(StringLen::N(36))", nullable)]
    pub run_id: Option<String>,
    /// 最近一次心跳时间；超过 `stale_after` 未更新视为实例失联。
    pub heartbeat_at: Option<DateTime>,
    /// 已开始执行的次数（含当前这次）。
    #[sea_orm(default_value = 0)]
    pub attempts: i32,
    pub created_at: DateTime,
    pub started_at: Option<DateTime>,
    pub finished_at: Option<DateTime>,
    pub cancel_requested: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// 创建任务领取与列表查询所需的索引。
///
/// 部分唯一索引必须使用原生 SQL：SeaORM 的 `Index` builder 不支持 `WHERE` 谓词。
/// 实体上 `kind` 已标记为 `indexed`，避免 schema sync 把这个部分唯一索引当成
/// 未声明的唯一索引而尝试删除（PostgreSQL 会报 constraint 不存在）。
pub async fn create_indexes(db: &DatabaseConnection) -> common_core::ContextualResult<()> {
    const STATEMENTS: &[&str] = &[
        // 同一 kind 最多一个活动任务：多实例去重的唯一正确性来源。
        // 活动状态集合需与 `AdminJobStatus::is_active` 保持一致。
        "CREATE UNIQUE INDEX IF NOT EXISTS uniq_admin_job_active_kind \
         ON admin_job (kind) \
         WHERE status IN ('queued', 'running', 'cancel_requested')",
        // 领取：按状态与创建顺序取最早的任务。
        "CREATE INDEX IF NOT EXISTS idx_admin_job_status_created_at \
         ON admin_job (status, created_at, id)",
        // 列表按 kind 过滤 + 游标。
        "CREATE INDEX IF NOT EXISTS idx_admin_job_kind_created_at \
         ON admin_job (kind, created_at, id)",
        // 回收心跳中断的任务。
        "CREATE INDEX IF NOT EXISTS idx_admin_job_heartbeat_at \
         ON admin_job (heartbeat_at) \
         WHERE status IN ('running', 'cancel_requested')",
    ];

    for statement in STATEMENTS {
        db.execute_unprepared(statement).await?;
    }
    Ok(())
}

/// 幂等注册索引回调，由 `types-db-init` 在 schema sync 之后调用。
#[common_macros::register_async(
    send,
    slice = types_db_registry::INIT_INDEXES,
    ty = types_db_registry::InitIndexFn
)]
async fn init_index(db: &DatabaseConnection) -> common_core::ContextualResult<()> {
    create_indexes(db).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Schema, Statement};

    /// 回归：`kind` 必须在实体上声明为 indexed。
    ///
    /// 否则 SeaORM 的 schema sync 会把原生创建的部分唯一索引
    /// `uniq_admin_job_active_kind` 视为未声明的唯一索引并尝试删除。
    #[test]
    fn kind_is_declared_indexed_for_schema_sync() {
        let indexes = Schema::new(DbBackend::Postgres).create_index_from_entity(Entity);
        let kind_column = Column::Kind.to_string();
        assert!(indexes.iter().any(|index| {
            let columns = index.get_index_spec().get_column_names();
            columns.len() == 1 && columns[0] == kind_column
        }));
    }

    /// 需要真实 PostgreSQL 验证 schema sync 不会删除部分唯一索引。
    ///
    /// ```text
    /// ADMIN_TEST_DB='postgres://user:pass@127.0.0.1:5432/memory_seek_test' \
    ///   cargo test -p types-admin --features orm -- --ignored
    /// ```
    #[tokio::test]
    #[ignore = "需要 ADMIN_TEST_DB 指向专用 PostgreSQL 测试库"]
    async fn schema_sync_keeps_partial_unique_index() {
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
        create_indexes(&db).await.unwrap();

        // 第二次同步必须保留原生创建的部分唯一索引，而不是尝试 DROP CONSTRAINT。
        db.get_schema_registry("types_admin")
            .sync(&db)
            .await
            .unwrap();

        let row = db
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT to_regclass('uniq_admin_job_active_kind') IS NOT NULL AS ok",
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(row.try_get::<bool>("", "ok").unwrap());

        db.execute_unprepared("DROP TABLE admin_job").await.unwrap();
    }
}
