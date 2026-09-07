use common::{
    DbConn,
    error::contextual::{Result, ext::IntoContextualExt},
    time::DateTime,
    types::CursorPage,
};
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Alias, Expr, Func},
};
use types::audit::{AuditId, AuditRecord};
use types::audit::{Column, Entity};

use types::auth::user::UserId;
use types::cursor::TimeIdCursor;
pub(super) struct AuditMapper;

impl AuditMapper {
    pub(super) async fn query_stats(
        db: &impl DbConn,
        event_type: Option<&str>,
        target_type: Option<&str>,
        start: Option<DateTime>,
        end: Option<DateTime>,
        trunc: &str,
    ) -> Result<Vec<(DateTime, i64)>> {
        let bucket = Expr::expr(
            Func::cust(Alias::new("date_trunc"))
                .arg(Expr::val(trunc))
                .arg(Expr::col((Entity, Column::CreatedAt))),
        );
        let mut query = Entity::find()
            .select_only()
            .expr_as(bucket.clone(), "bucket")
            .column_as(Column::EventId.count(), "cnt")
            .group_by(bucket)
            .order_by_asc(Column::CreatedAt);
        if let Some(event_type) = event_type {
            query = query.filter(Column::EventType.eq(event_type));
        }
        if let Some(target_type) = target_type {
            query = query.filter(Column::TargetType.eq(target_type));
        }
        if let Some(start) = start {
            query = query.filter(Column::CreatedAt.gte(start));
        }
        if let Some(end) = end {
            query = query.filter(Column::CreatedAt.lte(end));
        }
        query
            .into_tuple::<(DateTime, i64)>()
            .all(db)
            .await
            .into_contextual()
    }

    pub(super) async fn query_top_targets(
        db: &impl DbConn,
        event_type: &str,
        target_type: &str,
        limit: u64,
    ) -> Result<Vec<(i64, i64)>> {
        Entity::find()
            .select_only()
            .column(Column::TargetId)
            .column_as(Column::EventId.count(), "cnt")
            .filter(Column::EventType.eq(event_type))
            .filter(Column::TargetType.eq(target_type))
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
        db: &impl DbConn,
        event_type: Option<&str>,
        target_type: Option<&str>,
        target_id: Option<i64>,
        actor_id: Option<UserId>,
        cursor: &Option<TimeIdCursor<AuditId>>,
        size: u64,
    ) -> Result<CursorPage<AuditRecord, ()>> {
        let mut query = Entity::find()
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::EventId)
            .limit(size + 1);
        if let Some(event_type) = event_type {
            query = query.filter(Column::EventType.eq(event_type));
        }
        if let Some(target_type) = target_type {
            query = query.filter(Column::TargetType.eq(target_type));
        }
        if let Some(target_id) = target_id {
            query = query.filter(Column::TargetId.eq(target_id));
        }
        if let Some(actor_id) = actor_id {
            query = query.filter(Column::ActorId.eq(actor_id.0));
        }
        if let Some(cursor) = cursor {
            query = query.filter(cursor.before(Column::CreatedAt, Column::EventId));
        }

        let records = query
            .all(db)
            .await
            .into_contextual()?
            .into_iter()
            .map(AuditRecord::from)
            .collect::<Vec<_>>();

        Ok(CursorPage {
            has_more: records.len() > size as usize,
            records,
            next_cursor: None,
        })
    }
}
