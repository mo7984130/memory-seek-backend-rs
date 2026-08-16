#[cfg(feature = "enable")]
use chrono::{DateTime, Utc};
#[cfg(feature = "enable")]
use common::error::contextual::Result as ContextualResult;
#[cfg(feature = "enable")]
use common::ext::{IntoContextualExt, ToOk};
#[cfg(feature = "enable")]
use sea_orm::ConnectionTrait;
#[cfg(feature = "enable")]
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Alias, Expr, Func},
};
#[cfg(feature = "enable")]
use types::audit::BehaviorRecord;
#[cfg(feature = "enable")]
use types::audit::{Column, Entity};
#[cfg(feature = "enable")]
use types::auth::user::UserId;
#[cfg(feature = "enable")]
use types::cursor::TimeIdCursor;
#[cfg(feature = "enable")]
use types::photo::behavior::{BehaviorTargetType, UserBehaviorAction, UserBehaviorId};

#[cfg(feature = "enable")]
pub(super) struct AuditMapper;

#[cfg(feature = "enable")]
impl AuditMapper {
    pub(super) async fn query_stats(
        db: &impl ConnectionTrait,
        action: Option<UserBehaviorAction>,
        target_type: Option<BehaviorTargetType>,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
        trunc: &str,
    ) -> ContextualResult<Vec<(DateTime<Utc>, i64)>> {
        let bucket = Expr::expr(
            Func::cust(Alias::new("date_trunc"))
                .arg(Expr::val(trunc))
                .arg(Expr::col((Entity, Column::OccurredAt))),
        );
        let mut query = Entity::find()
            .select_only()
            .expr_as(bucket.clone(), "bucket")
            .column_as(Column::EventId.count(), "cnt")
            .group_by(bucket)
            .order_by_asc(Column::OccurredAt);
        if let Some(action) = action {
            query = query.filter(Column::EventType.eq(action.as_str()));
        }
        if let Some(target_type) = target_type {
            query = query.filter(Column::TargetType.eq(target_type.as_str()));
        }
        if let Some(start) = start {
            query = query.filter(Column::OccurredAt.gte(start));
        }
        if let Some(end) = end {
            query = query.filter(Column::OccurredAt.lte(end));
        }
        query
            .into_tuple::<(DateTime<Utc>, i64)>()
            .all(db)
            .await
            .into_contextual()
    }

    pub(super) async fn query_top_targets(
        db: &impl ConnectionTrait,
        action: UserBehaviorAction,
        target_type: BehaviorTargetType,
        limit: u64,
    ) -> ContextualResult<Vec<(i64, i64)>> {
        Entity::find()
            .select_only()
            .column(Column::TargetId)
            .column_as(Column::EventId.count(), "cnt")
            .filter(Column::EventType.eq(action.as_str()))
            .filter(Column::TargetType.eq(target_type.as_str()))
            .filter(Column::TargetId.is_not_null())
            .group_by(Column::TargetId)
            .order_by_desc(Column::EventId.count())
            .order_by_desc(Column::TargetId)
            .limit(limit)
            .into_tuple::<(i64, i64)>()
            .all(db)
            .await
            .into_contextual()
    }

    pub(super) async fn query_audit_page(
        db: &impl ConnectionTrait,
        action: Option<UserBehaviorAction>,
        target_type: Option<BehaviorTargetType>,
        target_id: Option<i64>,
        user_id: Option<UserId>,
        cursor: &Option<TimeIdCursor<UserBehaviorId>>,
        size: u64,
    ) -> ContextualResult<Vec<BehaviorRecord>> {
        let mut query = Entity::find()
            .order_by_desc(Column::OccurredAt)
            .order_by_desc(Column::EventId)
            .limit(size + 1);
        if let Some(action) = action {
            query = query.filter(Column::EventType.eq(action.as_str()));
        }
        if let Some(target_type) = target_type {
            query = query.filter(Column::TargetType.eq(target_type.as_str()));
        }
        if let Some(target_id) = target_id {
            query = query.filter(Column::TargetId.eq(target_id));
        }
        if let Some(user_id) = user_id {
            query = query.filter(Column::ActorId.eq(user_id.0));
        }
        if let Some(cursor) = cursor {
            query = query.filter(cursor.before(Column::OccurredAt, Column::EventId));
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
