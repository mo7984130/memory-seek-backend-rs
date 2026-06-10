use std::sync::Arc;

use axum::{Router, extract::State, routing::get};
use common::Result;
use common::ext::ResultRExt;
use common::r::R;

use crate::{
    models::timeline_stat::TimeRange, services::timeline_stat_service::TimelineStatService,
    state::PhotoState,
};

pub struct TimelineStatController;

impl TimelineStatController {
    pub fn protected_routes() -> Router<Arc<PhotoState>> {
        Router::new().route("/stats", get(Self::get_time_range))
    }

    pub fn public_routes() -> Router<Arc<PhotoState>> {
        Router::new()
    }

    async fn get_time_range(State(state): State<Arc<PhotoState>>) -> Result<R<TimeRange>> {
        TimelineStatService::get_time_range(&state).await.to_r_ok()
    }
}
