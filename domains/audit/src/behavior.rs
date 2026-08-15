//! 用户行为审计的持久化能力。
//!
//! `photo_user_behavior` 虽然由 photo 模块产生行为，但它是审计流水，
//! 因此实体、写入和查询都归审计域所有。photo 只负责组装行为请求。

use chrono::{DateTime, Utc};
use common::error::contextual::Result as ContextualResult;
#[cfg(feature = "persistence")]
use common::ext::{IntoContextualExt, ToOk};
use sea_orm::ConnectionTrait;
use sea_orm::entity::prelude::*;
#[cfg(feature = "persistence")]
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Statement,
};
use types::auth::user::UserId;
use types::cursor::TimeIdCursor;
use types::photo::behavior::{BehaviorTargetType, UserBehaviorAction, UserBehaviorId};
use types::photo::dto::behavior::{BehaviorAuditQuery, BehaviorStatsQuery, BehaviorTopQuery};

#[derive(Clone, Debug, PartialEq)]
pub struct BehaviorRecord {
    pub id: UserBehaviorId,
    pub user_id: UserId,
    pub action: UserBehaviorAction,
    pub target_type: Option<BehaviorTargetType>,
    pub target_id: Option<i64>,
    pub detail: Option<Json>,
    pub ip: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "photo_user_behavior")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: UserBehaviorId,
    pub user_id: UserId,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<i64>,
    #[sea_orm(column_type = "Json")]
    pub detail: Option<Json>,
    pub ip: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for BehaviorRecord {
    fn from(model: Model) -> Self {
        Self {
            id: model.id,
            user_id: model.user_id,
            action: model.action.parse().unwrap_or(UserBehaviorAction::View),
            target_type: model.target_type.as_deref().and_then(|s| s.parse().ok()),
            target_id: model.target_id,
            detail: model.detail,
            ip: model.ip,
            created_at: model.created_at,
        }
    }
}

struct BehaviorAuditStore;

/// 行为审计写入请求，由 audit domain 统一定义。
#[derive(Clone, Debug)]
pub struct BehaviorRecordReq {
    pub user_id: UserId,
    pub action: UserBehaviorAction,
    pub target_type: Option<BehaviorTargetType>,
    pub target_id: Option<i64>,
    pub detail: Option<Json>,
    pub ip: Option<String>,
}

impl BehaviorRecordReq {
    pub fn new(user_id: UserId, action: UserBehaviorAction) -> Self {
        Self {
            user_id,
            action,
            target_type: None,
            target_id: None,
            detail: None,
            ip: None,
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

    pub fn with_detail(mut self, detail: Json) -> Self {
        self.detail = Some(detail);
        self
    }

    pub fn with_ip(mut self, ip: Option<String>) -> Self {
        self.ip = ip;
        self
    }
}

impl BehaviorAuditStore {
    #[allow(clippy::too_many_arguments)]
    pub async fn insert(
        db: &impl ConnectionTrait,
        user_id: UserId,
        action: UserBehaviorAction,
        target_type: Option<BehaviorTargetType>,
        target_id: Option<i64>,
        detail: Option<Json>,
        ip: Option<&str>,
    ) -> ContextualResult<()> {
        #[cfg(not(feature = "persistence"))]
        {
            let _ = (db, user_id, action, target_type, target_id, detail, ip);
            return Ok(());
        }

        #[cfg(feature = "persistence")]
        Entity::insert(ActiveModel {
            user_id: Set(user_id),
            action: Set(action.as_str().to_string()),
            target_type: Set(target_type.map(|t| t.as_str().to_string())),
            target_id: Set(target_id),
            detail: Set(detail),
            ip: Set(ip.map(String::from)),
            created_at: Set(Utc::now()),
            ..Default::default()
        })
        .exec(db)
        .await
        .into_contextual()?;

        #[cfg(feature = "persistence")]
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn query_stats(
        db: &impl ConnectionTrait,
        action: Option<UserBehaviorAction>,
        target_type: Option<BehaviorTargetType>,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
        trunc: &str,
    ) -> ContextualResult<Vec<(DateTime<Utc>, i64)>> {
        #[cfg(not(feature = "persistence"))]
        {
            let _ = (db, action, target_type, start, end, trunc);
            return Ok(Vec::new());
        }

        #[cfg(feature = "persistence")]
        {
            let mut sql = format!(
                "SELECT date_trunc('{trunc}', created_at) AS bucket, COUNT(*) AS cnt \
                 FROM photo_user_behavior WHERE 1 = 1"
            );
            let mut binds: Vec<sea_orm::Value> = Vec::new();
            if let Some(action) = action {
                sql.push_str(" AND action = $");
                sql.push_str(&(binds.len() + 1).to_string());
                binds.push(action.as_str().into());
            }
            if let Some(target_type) = target_type {
                sql.push_str(" AND target_type = $");
                sql.push_str(&(binds.len() + 1).to_string());
                binds.push(target_type.as_str().into());
            }
            if let Some(start) = start {
                sql.push_str(" AND created_at >= $");
                sql.push_str(&(binds.len() + 1).to_string());
                binds.push(start.into());
            }
            if let Some(end) = end {
                sql.push_str(" AND created_at <= $");
                sql.push_str(&(binds.len() + 1).to_string());
                binds.push(end.into());
            }
            sql.push_str(" GROUP BY bucket ORDER BY bucket ASC");
            let rows = db
                .query_all(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    &sql,
                    binds,
                ))
                .await
                .into_contextual()?;
            rows.into_iter()
                .map(|row| Ok((row.try_get("", "bucket")?, row.try_get("", "cnt")?)))
                .collect::<ContextualResult<Vec<_>>>()
        }
    }

    pub async fn query_top_targets(
        db: &impl ConnectionTrait,
        action: UserBehaviorAction,
        target_type: BehaviorTargetType,
        limit: u64,
    ) -> ContextualResult<Vec<(i64, i64)>> {
        #[cfg(not(feature = "persistence"))]
        {
            let _ = (db, action, target_type, limit);
            return Ok(Vec::new());
        }

        #[cfg(feature = "persistence")]
        {
            let rows = db
                .query_all(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT target_id, COUNT(*) AS cnt FROM photo_user_behavior \
                     WHERE action = $1 AND target_type = $2 AND target_id IS NOT NULL \
                     GROUP BY target_id ORDER BY cnt DESC, target_id DESC LIMIT $3",
                    [
                        action.as_str().into(),
                        target_type.as_str().into(),
                        limit.into(),
                    ],
                ))
                .await
                .into_contextual()?;
            rows.into_iter()
                .map(|row| Ok((row.try_get("", "target_id")?, row.try_get("", "cnt")?)))
                .collect::<ContextualResult<Vec<_>>>()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn query_audit_page(
        db: &impl ConnectionTrait,
        action: Option<UserBehaviorAction>,
        target_type: Option<BehaviorTargetType>,
        target_id: Option<i64>,
        user_id: Option<UserId>,
        cursor: &Option<TimeIdCursor<UserBehaviorId>>,
        size: u64,
    ) -> ContextualResult<Vec<BehaviorRecord>> {
        #[cfg(not(feature = "persistence"))]
        {
            let _ = (db, action, target_type, target_id, user_id, cursor, size);
            return Ok(Vec::new());
        }

        #[cfg(feature = "persistence")]
        {
            let mut query = Entity::find()
                .order_by_desc(Column::CreatedAt)
                .order_by_desc(Column::Id)
                .limit(size + 1);
            if let Some(action) = action {
                query = query.filter(Column::Action.eq(action.as_str()));
            }
            if let Some(target_type) = target_type {
                query = query.filter(Column::TargetType.eq(target_type.as_str()));
            }
            if let Some(target_id) = target_id {
                query = query.filter(Column::TargetId.eq(target_id));
            }
            if let Some(user_id) = user_id {
                query = query.filter(Column::UserId.eq(user_id));
            }
            if let Some(c) = cursor {
                query = query.filter(c.before(Column::CreatedAt, Column::Id));
            }
            query
                .all(db)
                .await
                .into_contextual()?
                .into_iter()
                .map(BehaviorRecord::from)
                .collect::<Vec<_>>()
                .to_ok()
        }
    }
}

impl super::AuditService {
    /// 记录行为但不阻断业务响应，供非事务型调用点使用。
    pub async fn record_behavior_best_effort(db: &impl ConnectionTrait, req: BehaviorRecordReq) {
        let _ = Self::record_behavior(db, req).await;
    }

    pub async fn record_behavior(
        db: &impl ConnectionTrait,
        req: BehaviorRecordReq,
    ) -> ContextualResult<()> {
        BehaviorAuditStore::insert(
            db,
            req.user_id,
            req.action,
            req.target_type,
            req.target_id,
            req.detail,
            req.ip.as_deref(),
        )
        .await
    }

    pub async fn query_behavior_stats(
        db: &impl ConnectionTrait,
        req: &BehaviorStatsQuery,
    ) -> ContextualResult<Vec<(DateTime<Utc>, i64)>> {
        BehaviorAuditStore::query_stats(
            db,
            req.action,
            req.target_type,
            req.start,
            req.end,
            req.granularity.as_trunc(),
        )
        .await
    }

    pub async fn query_behavior_top(
        db: &impl ConnectionTrait,
        req: &BehaviorTopQuery,
    ) -> ContextualResult<Vec<(i64, i64)>> {
        BehaviorAuditStore::query_top_targets(db, req.action, req.target_type, req.limit).await
    }

    pub async fn query_behavior_audit(
        db: &impl ConnectionTrait,
        req: &BehaviorAuditQuery,
    ) -> ContextualResult<Vec<BehaviorRecord>> {
        BehaviorAuditStore::query_audit_page(
            db,
            req.action,
            req.target_type,
            req.target_id,
            req.user_id,
            &req.cursor,
            req.size,
        )
        .await
    }
}
