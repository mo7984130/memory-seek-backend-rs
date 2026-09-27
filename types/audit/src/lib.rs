//! 审计事件类型、查询 DTO 与 `audit_event` SeaORM 实体。

use common_core::time::{DateTime, now};
use serde_json::Value;

use types_core::cursor::TimeIdCursor;

mod model;
pub use model::*;

pub use types_core::AuditId;
/// DTO 声明宏(定义在共享内核 `types-core`,此处重导出以便本 crate 的 DTO 使用
/// `crate::in_dto!` / `crate::out_dto!` 路径)。
pub use types_core::{in_dto, out_dto};

/// schema 前缀自我登记: `init_db` 按前缀逐个同步表结构。
#[cfg(feature = "orm")]
#[linkme::distributed_slice(types_db_registry::SCHEMA_PREFIXES)]
static SCHEMA_PREFIX: types_db_registry::SchemaPrefix =
    types_db_registry::SchemaPrefix("types_audit");

/// 一个必须和业务状态一起提交的审计事实。
#[derive(Clone, Debug, PartialEq)]
pub struct AuditEvent {
    pub event_id: AuditId,
    pub event_type: String,
    pub actor_id: Option<i64>,
    pub target_type: Option<String>,
    pub target_id: Option<i64>,
    pub detail: Option<Value>,
    pub occurred_at: DateTime,
}

impl AuditEvent {
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_id: AuditId(0),
            event_type: event_type.into(),
            actor_id: None,
            target_type: None,
            target_id: None,
            detail: None,
            occurred_at: now(),
        }
    }
    pub fn with_actor(mut self, actor_id: impl Into<i64>) -> Self {
        self.actor_id = Some(actor_id.into());
        self
    }
    pub fn with_target(
        mut self,
        target_type: impl Into<String>,
        target_id: impl Into<i64>,
    ) -> Self {
        self.target_type = Some(target_type.into());
        self.target_id = Some(target_id.into());
        self
    }
    pub fn with_detail(mut self, detail: Value) -> Self {
        self.detail = Some(detail);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuditRecord {
    pub id: AuditId,
    pub event_type: String,
    pub actor_id: Option<i64>,
    pub target_type: Option<String>,
    pub target_id: Option<i64>,
    pub detail: Option<Value>,
    pub created_at: DateTime,
}

impl AuditRecord {
    pub fn cursor(&self) -> TimeIdCursor<AuditId> {
        TimeIdCursor {
            time_at: self.created_at,
            id: self.id,
        }
    }
}

#[cfg(feature = "orm")]
mod entity {
    use super::*;

    use common_core::time::DateTime;
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "audit_event")]
    pub struct Model {
        /// 主键
        #[sea_orm(primary_key)]
        pub event_id: AuditId,

        /// 事件类型
        /// 索引, 用于 根据事件类型获取事件列表
        #[sea_orm(indexed)]
        pub event_type: String,

        /// 发起者ID
        /// 索引, 用于 根据发起者获取事件列表
        #[sea_orm(indexed)]
        pub actor_id: Option<i64>,

        /// 目标类型
        pub target_type: Option<String>,

        /// 目标ID
        pub target_id: Option<i64>,

        /// 详细信息
        #[sea_orm(column_type = "Json")]
        pub detail: Option<Json>,

        /// 创建时间
        /// 索引, 用于 按时间排序获取事件列表
        #[sea_orm(indexed)]
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub created_at: DateTime,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "orm")]
pub use entity::*;

#[cfg(feature = "orm")]
impl From<Model> for AuditRecord {
    fn from(model: Model) -> Self {
        Self {
            id: model.event_id,
            event_type: model.event_type,
            actor_id: model.actor_id,
            target_type: model.target_type,
            target_id: model.target_id,
            detail: model.detail,
            created_at: model.created_at,
        }
    }
}
