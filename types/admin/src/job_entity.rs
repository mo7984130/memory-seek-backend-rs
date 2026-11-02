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
