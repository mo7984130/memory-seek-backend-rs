//! 审计事件类型、查询 DTO 与 `audit_event` SeaORM 实体。

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::auth::user::UserId;
use crate::cursor::TimeIdCursor;
use crate::photo::behavior::{BehaviorTargetType, UserBehaviorAction, UserBehaviorId};

mod model;
pub use model::*;

/// 一个必须和业务状态一起提交的审计事实。
#[derive(Clone, Debug, PartialEq)]
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
            event_id: 0,
            event_type: event_type.into(),
            actor_id: None,
            target_type: None,
            target_id: None,
            detail: None,
            occurred_at: Utc::now(),
        }
    }
    pub fn with_id(mut self, event_id: i64) -> Self {
        self.event_id = event_id;
        self
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

#[derive(Clone, Debug)]
pub struct BehaviorRecordReq {
    pub user_id: UserId,
    pub action: UserBehaviorAction,
    pub target_type: Option<BehaviorTargetType>,
    pub target_id: Option<i64>,
    pub detail: Option<Value>,
}

impl BehaviorRecordReq {
    pub fn new(user_id: UserId, action: UserBehaviorAction) -> Self {
        Self {
            user_id,
            action,
            target_type: None,
            target_id: None,
            detail: None,
        }
    }
    pub fn with_photo(mut self, photo_id: i64) -> Self {
        self.target_type = Some(BehaviorTargetType::Photo);
        self.target_id = Some(photo_id);
        self
    }
    pub fn with_target(mut self, target_type: BehaviorTargetType, target_id: i64) -> Self {
        self.target_type = Some(target_type);
        self.target_id = Some(target_id);
        self
    }
    pub fn with_detail(mut self, detail: Value) -> Self {
        self.detail = Some(detail);
        self
    }
    pub fn into_event(self, event_id: i64) -> AuditEvent {
        AuditEvent::new(self.action.as_str())
            .with_id(event_id)
            .with_actor(self.user_id.0)
            .with_detail_option(self.detail)
            .with_optional_target(self.target_type, self.target_id)
    }
}

impl AuditEvent {
    fn with_detail_option(mut self, detail: Option<Value>) -> Self {
        self.detail = detail;
        self
    }
    fn with_optional_target(
        mut self,
        target_type: Option<BehaviorTargetType>,
        target_id: Option<i64>,
    ) -> Self {
        self.target_type = target_type.map(|value| value.as_str().to_owned());
        self.target_id = target_id;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BehaviorRecord {
    pub id: UserBehaviorId,
    pub user_id: UserId,
    pub action: UserBehaviorAction,
    pub target_type: Option<BehaviorTargetType>,
    pub target_id: Option<i64>,
    pub detail: Option<Value>,
    pub created_at: DateTime<Utc>,
}

impl BehaviorRecord {
    pub fn cursor(&self) -> TimeIdCursor<UserBehaviorId> {
        TimeIdCursor {
            created_at: self.created_at,
            id: self.id,
        }
    }
}

#[cfg(feature = "orm")]
mod entity {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
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
        pub occurred_at: DateTimeUtc,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "orm")]
pub use entity::*;

#[cfg(feature = "orm")]
impl From<Model> for BehaviorRecord {
    fn from(model: Model) -> Self {
        Self {
            id: UserBehaviorId(model.event_id),
            user_id: UserId(model.actor_id.unwrap_or_default()),
            action: model.event_type.parse().unwrap_or(UserBehaviorAction::View),
            target_type: model
                .target_type
                .as_deref()
                .and_then(|value| value.parse().ok()),
            target_id: model.target_id,
            detail: model.detail,
            created_at: model.occurred_at,
        }
    }
}
