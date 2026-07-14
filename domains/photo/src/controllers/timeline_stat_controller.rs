use std::sync::Arc;

use axum::{Router, extract::State, routing::get};
use common::Result;
use common::ext::ResultRExt;
use common::r::R;

use crate::{
    models::timeline_stat::MonthStat, services::timeline_stat_service::TimelineStatService,
    state::PhotoState,
};

pub struct TimelineStatController;

impl TimelineStatController {
    pub fn protected_routes() -> Router<Arc<PhotoState>> {
        Router::new().route("/stats", get(Self::get_monthly_stats))
    }

    pub fn public_routes() -> Router<Arc<PhotoState>> {
        Router::new()
    }

    async fn get_monthly_stats(State(state): State<Arc<PhotoState>>) -> Result<R<Vec<MonthStat>>> {
        TimelineStatService::get_monthly_stats(&state)
            .await
            .to_r_ok()
    }
}
