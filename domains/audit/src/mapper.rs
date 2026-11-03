use common_core::{
    DbConn,
    error::contextual::{Result, ext::IntoContextualExt},
    time::DateTime,
    types::CursorPage,
};
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Alias, Expr, Func},
};
use types_audit::{AuditQuery, AuditRecord};
use types_audit::{Column, Entity};

pub(super) struct AuditMapper;

impl AuditMapper {
    /// 按时间桶统计审计事件数量
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
            // ORDER BY 必须使用聚合后的时间桶，不能直接用未分组的 created_at。
            .order_by_asc(Expr::col(Alias::new("bucket")));
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

    /// 统计事件数量最多的目标并限制返回条数
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

    /// 游标分页查询审计事件记录
    pub(super) async fn query_audit_page(
        db: &impl DbConn,
        req: &AuditQuery,
    ) -> Result<CursorPage<AuditRecord, ()>> {
        let mut base_query = Entity::find();
        if let Some(event_type) = req.event_type.as_deref() {
            base_query = base_query.filter(Column::EventType.eq(event_type));
        }
        if let Some(target_type) = req.target_type.as_deref() {
            base_query = base_query.filter(Column::TargetType.eq(target_type));
        }
        if let Some(target_id) = req.target_id {
            base_query = base_query.filter(Column::TargetId.eq(target_id));
        }
        if let Some(actor_id) = req.actor_id {
            base_query = base_query.filter(Column::ActorId.eq(actor_id.0));
        }
        let mut query = base_query
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::EventId)
            .limit(req.size + 1);
        if let Some(cursor) = &req.cursor {
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
            has_more: records.len() > req.size as usize,
            records,
            next_cursor: None,
        })
    }
}
