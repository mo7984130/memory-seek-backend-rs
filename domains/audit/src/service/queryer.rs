use common::utils::MetricsTimerExt;
use common::{DbConn, Result, ext::ToOk, metrics_name, types::CursorPage};
use types_audit::{
    AuditId, AuditItem, AuditQuery, AuditStatsItem, AuditStatsQuery, AuditTopItem, AuditTopQuery,
};
use types_core::cursor::TimeIdCursor;

use crate::mapper::AuditMapper;

pub struct AuditQueryer;

impl AuditQueryer {
    #[common_macros::metered]
    #[tracing::instrument(skip_all)]
    pub async fn query_stats(
        db: &impl DbConn,
        req: &AuditStatsQuery,
    ) -> Result<Vec<AuditStatsItem>> {
        Ok(AuditMapper::query_stats(
            db,
            req.event_type.as_deref(),
            req.target_type.as_deref(),
            req.start,
            req.end,
            req.granularity.as_trunc(),
        )
        .timed(metrics_name!("db_query"))
        .await?
        .into_iter()
        .map(|(bucket, count)| AuditStatsItem { bucket, count })
        .collect())
    }

    #[common_macros::metered]
    #[tracing::instrument(skip_all)]
    pub async fn query_top(db: &impl DbConn, req: &AuditTopQuery) -> Result<Vec<AuditTopItem>> {
        Ok(
            AuditMapper::query_top_targets(db, &req.event_type, &req.target_type, req.limit)
                .timed(metrics_name!("db_query"))
                .await?
                .into_iter()
                .map(|(target_id, count)| AuditTopItem { target_id, count })
                .collect(),
        )
    }

    #[common_macros::metered]
    #[tracing::instrument(skip_all)]
    pub async fn query_events(
        db: &impl DbConn,
        req: &AuditQuery,
    ) -> Result<CursorPage<AuditItem, TimeIdCursor<AuditId>>> {
        AuditMapper::query_audit_page(
            db,
            req.event_type.as_deref(),
            req.target_type.as_deref(),
            req.target_id,
            req.actor_id,
            &req.cursor,
            req.size,
        )
        .timed(metrics_name!("db_query"))
        .await?
        .map_records(|records| records.into_iter().map(AuditItem::from).collect())
        .with_next_cursor(|record| TimeIdCursor {
            time_at: record.created_at,
            id: record.id,
        })
        .to_ok()
    }
}
