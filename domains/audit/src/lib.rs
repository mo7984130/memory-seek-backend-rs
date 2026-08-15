//! 事务强一致审计域。
//!
//! 审计域不创建或管理事务。调用方必须把 [`AuditService::append`] 放在自己的
//! `DatabaseTransaction` 中，审计写入失败时由调用方事务统一回滚。

use chrono::{DateTime, Utc};
#[cfg(feature = "persistence")]
use common::error::{AppError, ContextualError};
use sea_orm::DatabaseTransaction;
#[cfg(feature = "persistence")]
use sea_orm::{ActiveValue::Set, EntityTrait};
use serde_json::Value;

mod behavior;
mod snowflake;

pub use behavior::{BehaviorRecord, BehaviorRecordReq};
#[cfg(feature = "controller")]
pub mod controller;
#[cfg(feature = "controller")]
pub use controller::AuditController;

#[derive(Clone)]
#[cfg(feature = "controller")]
pub struct AuditState {
    pub db: sea_orm::DatabaseConnection,
}

#[cfg(feature = "persistence")]
mod mapper {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "audit_event")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub event_id: i64,
        pub event_type: String,
        pub actor_id: Option<i64>,
        pub target_type: Option<String>,
        pub target_id: Option<i64>,
        #[sea_orm(column_type = "Json")]
        pub detail: Option<Json>,
        pub occurred_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "persistence")]
use mapper::Entity;

/// 一个必须和业务状态一起提交的审计事实。
#[derive(Clone, Debug)]
pub struct AuditEvent {
    pub event_id: i64,
    pub event_type: String,
    pub actor_id: Option<i64>,
    pub target_type: Option<String>,
    pub target_id: Option<i64>,
    pub detail: Option<Value>,
    pub occurred_at: DateTime<Utc>,
}

impl AuditEvent {
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_id: snowflake::next_id(),
            event_type: event_type.into(),
            actor_id: None,
            target_type: None,
            target_id: None,
            detail: None,
            occurred_at: Utc::now(),
        }
    }

    pub fn with_actor(mut self, actor_id: i64) -> Self {
        self.actor_id = Some(actor_id);
        self
    }

    pub fn with_target(mut self, target_type: impl Into<String>, target_id: i64) -> Self {
        self.target_type = Some(target_type.into());
        self.target_id = Some(target_id);
        self
    }

    pub fn with_detail(mut self, detail: Value) -> Self {
        self.detail = Some(detail);
        self
    }
}

pub struct AuditService;

impl AuditService {
    /// 在调用方当前事务中追加审计事实。
    pub async fn append(txn: &DatabaseTransaction, event: AuditEvent) -> common::Result<()> {
        #[cfg(not(feature = "persistence"))]
        {
            let _ = (txn, event);
            return Ok(());
        }

        #[cfg(feature = "persistence")]
        Entity::insert(mapper::ActiveModel {
            event_id: Set(event.event_id),
            event_type: Set(event.event_type),
            actor_id: Set(event.actor_id),
            target_type: Set(event.target_type),
            target_id: Set(event.target_id),
            detail: Set(event.detail),
            occurred_at: Set(event.occurred_at),
        })
        .exec(txn)
        .await
        .map_err(|error| {
            ContextualError::error(
                "audit_event_insert",
                "写入审计事件失败",
                error,
                AppError::InternalServerError,
            )
        })?;

        #[cfg(feature = "persistence")]
        Ok(())
    }
}
