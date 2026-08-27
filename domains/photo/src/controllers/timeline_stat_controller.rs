use std::sync::Arc;

use axum::{Router, extract::State, routing::get};
use common::Result;
use common::axum::{R, ext::ToROkExt};
use types::photo::dto::timeline_stat::MonthStat;

use crate::{services::timeline_stat_service::TimelineStatService, state::PhotoState};

pub struct TimelineStatController;

impl TimelineStatController {
    pub fn protected_routes() -> Router<Arc<PhotoState>> {
        Router::new().route("/stats", get(Self::get_monthly_stats))
    }

    pub fn public_routes() -> Router<Arc<PhotoState>> {
        Router::new()
    }

    /// 返回按月份聚合的照片时间线统计.
    async fn get_monthly_stats(State(state): State<Arc<PhotoState>>) -> Result<R<Vec<MonthStat>>> {
        TimelineStatService::get_monthly_stats(&state)
            .await
            .to_r_ok()
    }
}
