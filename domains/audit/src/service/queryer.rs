use std::time::Duration;

use common_core::{DbConn, Result, error::ContextualError, ext::ToOk, types::CursorPage};
use common_metrics::{MetricsTimerExt, metrics_name};
use multi_level_cache::MultiLevelCache;
use types_audit::{
    AuditId, AuditItem, AuditQuery, AuditStatsItem, AuditStatsQuery, AuditTopItem, AuditTopQuery,
};
use types_core::{AdminId, cursor::TimeIdCursor};

use crate::mapper::AuditMapper;

pub struct AuditQueryer;

const AUDIT_CURSOR_COUNT_CACHE_TTL: Duration = Duration::from_secs(30);

impl AuditQueryer {
    /// 查询审计事件统计数据
    #[common_macros::metered]
    #[tracing::instrument(skip_all)]
    pub async fn query_stats(
        db: &impl DbConn,
        _admin: AdminId,
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

    /// 查询事件数量最多的目标排行
    #[common_macros::metered]
    #[tracing::instrument(skip_all)]
    pub async fn query_top(
        db: &impl DbConn,
        _admin: AdminId,
        req: &AuditTopQuery,
    ) -> Result<Vec<AuditTopItem>> {
        Ok(
            AuditMapper::query_top_targets(db, &req.event_type, &req.target_type, req.limit)
                .timed(metrics_name!("db_query"))
                .await?
                .into_iter()
                .map(|(target_id, count)| AuditTopItem { target_id, count })
                .collect(),
        )
    }

    /// 游标分页查询审计事件列表
    #[common_macros::metered]
    #[tracing::instrument(skip_all)]
    pub async fn query_events(
        db: &impl DbConn,
        _admin: AdminId,
        req: &AuditQuery,
        count_cache: &MultiLevelCache<u64, ContextualError>,
    ) -> Result<CursorPage<AuditItem, TimeIdCursor<AuditId>>> {
        let key_material = format!(
            "{:?}|{:?}|{:?}|{:?}",
            req.event_type, req.target_type, req.target_id, req.actor_id
        );
        let count_key = format!(
            "a:audit:count:{}",
            blake3::hash(key_material.as_bytes()).to_hex()
        );
        let total_count = count_cache
            .get_or_load(&count_key, AUDIT_CURSOR_COUNT_CACHE_TTL, || async {
                AuditMapper::query_audit_count(db, req).await
            })
            .await?;
        AuditMapper::query_audit_page(db, req, total_count)
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
