use std::sync::Arc;

use axum::{Router, extract::State, routing::get};
use common::Result;
use common::axum::{R, ext::ToROkExt};
use types::media::dto::timeline_stat::MonthStat;

use crate::{services::timeline_stat_service::TimelineStatService, state::MediaState};

pub struct TimelineStatController;

impl TimelineStatController {
    pub fn protected_routes() -> Router<Arc<MediaState>> {
        Router::new().route("/stats", get(Self::get_monthly_stats))
    }

    pub fn public_routes() -> Router<Arc<MediaState>> {
        Router::new()
    }

    /// 返回按月份聚合的照片时间线统计.
    async fn get_monthly_stats(State(state): State<Arc<MediaState>>) -> Result<R<Vec<MonthStat>>> {
        TimelineStatService::get_monthly_stats(&state)
            .await
            .to_r_ok()
    }
}
