use common::ext::OkExt;
use common::models::CursorPage;
use common::{Result, ext::CollectOkExt};
use sea_orm::ConnectionTrait;
use types::audit::{
    AuditEventId, AuditItem, AuditQuery, AuditStatsItem, AuditStatsQuery, AuditTopItem,
    AuditTopQuery,
};
use types::cursor::TimeIdCursor;

use crate::mapper::AuditMapper;

pub struct AuditQueryer;

impl AuditQueryer {
    pub async fn query_stats(
        db: &impl ConnectionTrait,
        req: &AuditStatsQuery,
    ) -> Result<Vec<AuditStatsItem>> {
        AuditMapper::query_stats(
            db,
            req.event_type.as_deref(),
            req.target_type.as_deref(),
            req.start,
            req.end,
            req.granularity.as_trunc(),
        )
        .await?
        .into_iter()
        .map(|(bucket, count)| AuditStatsItem { bucket, count })
        .collect_ok()
    }

    pub async fn query_top(
        db: &impl ConnectionTrait,
        req: &AuditTopQuery,
    ) -> Result<Vec<AuditTopItem>> {
        AuditMapper::query_top_targets(db, &req.event_type, &req.target_type, req.limit)
            .await?
            .into_iter()
            .map(|(target_id, count)| AuditTopItem { target_id, count })
            .collect_ok()
    }

    pub async fn query_events(
        db: &impl ConnectionTrait,
        req: &AuditQuery,
    ) -> Result<CursorPage<AuditItem, TimeIdCursor<AuditEventId>>> {
        AuditMapper::query_audit_page(
            db,
            req.event_type.as_deref(),
            req.target_type.as_deref(),
            req.target_id,
            req.actor_id,
            &req.cursor,
            req.size,
        )
        .await?
        .map_records(|records| records.into_iter().map(AuditItem::from).collect())
        .with_next_cursor(|record| TimeIdCursor {
            time_at: record.created_at,
            id: record.id,
        })
        .to_ok()
    }
}
